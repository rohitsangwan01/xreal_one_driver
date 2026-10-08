use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use std::net::SocketAddr;
use std::{ffi::CStr, io::Read, net::TcpStream, os::raw::c_char, time::Duration};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct XOImu {
    pub gyro: [f32; 3],
    pub accel: [f32; 3],
    pub timestamp: u64,
}

pub struct XrealOne {
    stream: TcpStream,
    recv_buffer: Vec<u8>,
}

impl XrealOne {
    const HEADER: [u8; 6] = [0x28, 0x36, 0x00, 0x00, 0x00, 0x80];
    // Starts at byte 79. Byte 78 is part of the frame counter, which passes 2^24
    // about 4 hours 40 minutes after the glasses boot.
    const SENSOR: [u8; 5] = [0x40, 0x1f, 0x00, 0x00, 0x40];

    pub fn new() -> Result<Self, std::io::Error> {
        let addr = "169.254.2.1:52998";
        Self::new_with_addr(addr)
    }

    pub fn new_with_addr(addr: &str) -> Result<Self, std::io::Error> {
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        let stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self {
            stream,
            recv_buffer: Vec::new(),
        })
    }

    pub fn next(&mut self) -> Result<XOImu, std::io::Error> {
        loop {
            if let Some(imu) = self.try_parse_message() {
                return Ok(imu);
            }
            let mut temp_buf = [0u8; 4096];
            match self.stream.read(&mut temp_buf) {
                Ok(0) => {
                    println!("Connection closed");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Connection closed",
                    ));
                }
                Ok(n) => self.recv_buffer.extend_from_slice(&temp_buf[..n]),
                Err(e) => {
                    println!("Error reading glasses event: {}", e);
                    return Err(e);
                }
            }
        }
    }

    fn try_parse_message(&mut self) -> Option<XOImu> {
        Self::parse_buffer(&mut self.recv_buffer)
    }

    fn parse_buffer(recv_buffer: &mut Vec<u8>) -> Option<XOImu> {
        loop {
            let header_pos = match Self::find_subsequence(recv_buffer, &Self::HEADER) {
                Some(pos) => pos,
                None => return None,
            };
            if header_pos > 0 {
                recv_buffer.drain(..header_pos);
            }
            const MIN_MESSAGE_SIZE: usize = 84;
            if recv_buffer.len() < MIN_MESSAGE_SIZE {
                return None;
            }
            let slice = &recv_buffer[..MIN_MESSAGE_SIZE];

            if Self::find_subsequence(slice, &Self::SENSOR).is_none() {
                recv_buffer.drain(..Self::HEADER.len());
                continue;
            }

            match Self::try_decode_imu(slice) {
                Ok(imu) => {
                    recv_buffer.drain(..MIN_MESSAGE_SIZE);
                    return Some(imu);
                }
                Err(_) => {
                    recv_buffer.drain(..Self::HEADER.len());
                }
            }
        }
    }

    fn try_decode_imu(data: &[u8]) -> Result<XOImu, std::io::Error> {
        let mut reader = Cursor::new(data);

        reader.set_position(14);
        let ts1 = reader.read_u64::<LittleEndian>()? / 1000;

        reader.set_position(34);
        let gx = reader.read_f32::<LittleEndian>()?;
        let gy = reader.read_f32::<LittleEndian>()?;
        let gz = reader.read_f32::<LittleEndian>()?;

        let ax = reader.read_f32::<LittleEndian>()?;
        let ay = reader.read_f32::<LittleEndian>()?;
        let az = reader.read_f32::<LittleEndian>()?;

        if gx.is_nan() || gy.is_nan() || gz.is_nan() || ax.is_nan() || ay.is_nan() || az.is_nan() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "NaN values in sensor data",
            ));
        }

        if gx.is_infinite()
            || gy.is_infinite()
            || gz.is_infinite()
            || ax.is_infinite()
            || ay.is_infinite()
            || az.is_infinite()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Infinite values in sensor data",
            ));
        }

        // Check for extreme values that indicate corrupted data
        let max_reasonable_gyro = 1000.0;
        let max_reasonable_accel = 100.0;

        if gx.abs() > max_reasonable_gyro
            || gy.abs() > max_reasonable_gyro
            || gz.abs() > max_reasonable_gyro
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Extreme gyroscope values detected",
            ));
        }

        if ax.abs() > max_reasonable_accel
            || ay.abs() > max_reasonable_accel
            || az.abs() > max_reasonable_accel
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Extreme accelerometer values detected",
            ));
        }

        // Check for suspicious patterns (all zeros, all same value, etc.)
        let gyro_magnitude = (gx * gx + gy * gy + gz * gz).sqrt();
        let accel_magnitude = (ax * ax + ay * ay + az * az).sqrt();

        if gyro_magnitude < 1e-6 && accel_magnitude < 1e-6 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Suspicious zero sensor data",
            ));
        }

        if accel_magnitude < 5.0 || accel_magnitude > 15.0 {
            // Normal gravity should be around 9.8 m/s², allow some variation
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unreasonable accelerometer magnitude",
            ));
        }

        Ok(XOImu {
            gyro: [-gx, -gz, -gy],
            accel: [-ax, -az, -ay],
            timestamp: ts1,
        })
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }
}

// ===================== C FFI =====================

#[repr(C)]
pub struct XrealOneHandle {
    inner: XrealOne,
}

#[no_mangle]
pub extern "C" fn xo_new() -> *mut XrealOneHandle {
    match XrealOne::new() {
        Ok(inner) => Box::into_raw(Box::new(XrealOneHandle { inner })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn xo_new_with_addr(addr: *const c_char) -> *mut XrealOneHandle {
    if addr.is_null() {
        return std::ptr::null_mut();
    }
    let cstr = unsafe { CStr::from_ptr(addr) };
    let s = match cstr.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match XrealOne::new_with_addr(s) {
        Ok(inner) => Box::into_raw(Box::new(XrealOneHandle { inner })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn xo_free(handle: *mut XrealOneHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

// Returns 0 on success, non-zero on failure
#[no_mangle]
pub extern "C" fn xo_next(handle: *mut XrealOneHandle, out: *mut XOImu) -> i32 {
    if handle.is_null() || out.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *handle };
    match handle.inner.next() {
        Ok(xo) => {
            unsafe {
                *out = xo;
            }
            0
        }
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::XrealOne;

    // From an XREAL One Pro. The frame counter is over 2^24, so its top byte (byte 78) isn't 0.
    const IMU_FRAME_ID_ABOVE_2_24: [u8; 134] = [
        0x28, 0x36, 0x00, 0x00, 0x00, 0x80, 0x38, 0x49, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0xcb, 0x24,
        0x24, 0xf3, 0xa7, 0x18, 0x00, 0x00, 0x5e, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0b, 0x00,
        0x00, 0x00, 0x0b, 0x59, 0x5c, 0x3c, 0x58, 0xa0, 0x0b, 0xbd, 0x81, 0x13, 0x4d, 0xbc, 0x33, 0x83,
        0xfa, 0xbe, 0x33, 0xf2, 0x05, 0xc1, 0x9a, 0xe0, 0xa4, 0x40, 0x00, 0x00, 0x48, 0xc5, 0x00, 0x00,
        0x48, 0xc5, 0x00, 0x00, 0x48, 0xc5, 0x00, 0x00, 0x2b, 0x42, 0x02, 0xdd, 0x9f, 0x9d, 0x01, 0x40,
        0x1f, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0,
        0x7e, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x88, 0xc5, 0x62, 0xb2, 0x7f, 0x00, 0x00, 0x00, 0xd0, 0x54,
        0xa6, 0xb2, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const MAG_FRAME: [u8; 134] = [
        0x28, 0x36, 0x00, 0x00, 0x00, 0x80, 0x38, 0x49, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x59, 0xd1,
        0x34, 0xf3, 0xa7, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xc0, 0x7f, 0x00, 0x00, 0xc0, 0x7f, 0x00, 0x00, 0xc0, 0x7f, 0x00, 0x00,
        0xc0, 0x7f, 0x00, 0x00, 0xc0, 0x7f, 0x00, 0x00, 0xc0, 0x7f, 0x00, 0x00, 0x61, 0x41, 0x00, 0x20,
        0x99, 0xc0, 0x00, 0x80, 0xd4, 0xc0, 0x00, 0x00, 0xc8, 0x41, 0x02, 0xf2, 0x70, 0xa5, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88, 0xc5, 0x62, 0xb2, 0x7f, 0x00, 0x00, 0x00, 0xd0, 0x54,
        0xa6, 0xb2, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    fn with_frame_id_top_byte(frame: &[u8], top: u8) -> Vec<u8> {
        let mut f = frame.to_vec();
        f[78] = top;
        f
    }

    #[test]
    fn accepts_imu_frame_with_frame_id_above_2_24() {
        let mut buf = IMU_FRAME_ID_ABOVE_2_24.to_vec();
        let imu = XrealOne::parse_buffer(&mut buf).expect("IMU frame should parse");
        let accel = (imu.accel[0].powi(2) + imu.accel[1].powi(2) + imu.accel[2].powi(2)).sqrt();
        assert!((accel - 9.8).abs() < 0.5, "accel magnitude {}", accel);
    }

    #[test]
    fn accepts_imu_frame_with_frame_id_below_2_24() {
        let mut buf = with_frame_id_top_byte(&IMU_FRAME_ID_ABOVE_2_24, 0x00);
        assert!(XrealOne::parse_buffer(&mut buf).is_some());
    }

    #[test]
    fn skips_magnetometer_frame() {
        let mut buf = MAG_FRAME.to_vec();
        buf.extend_from_slice(&IMU_FRAME_ID_ABOVE_2_24);
        assert!(XrealOne::parse_buffer(&mut buf).is_some());
        // Only one sample: the magnetometer frame is skipped.
        assert!(XrealOne::parse_buffer(&mut buf).is_none());
    }

    #[test]
    fn resyncs_after_leading_garbage() {
        let mut buf = vec![0xaa, 0x28, 0x36, 0x01];
        buf.extend_from_slice(&IMU_FRAME_ID_ABOVE_2_24);
        assert!(XrealOne::parse_buffer(&mut buf).is_some());
    }
}
