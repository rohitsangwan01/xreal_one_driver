use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use std::net::SocketAddr;
use std::{ffi::CStr, io::Read, net::TcpStream, os::raw::c_char, time::Duration};

#[derive(Debug, Clone)]
pub struct IMUData {
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
    const SENSOR: [u8; 6] = [0x00, 0x40, 0x1f, 0x00, 0x00, 0x40];

    pub fn new() -> Result<Self, std::io::Error> {
        let addr = "169.254.2.1:52998";
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        let stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self {
            stream,
            recv_buffer: Vec::new(),
        })
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

    pub fn next(&mut self) -> std::io::Result<IMUData> {
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

    fn try_parse_message(&mut self) -> Option<IMUData> {
        loop {
            let header_pos = match Self::find_subsequence(&self.recv_buffer, &Self::HEADER) {
                Some(pos) => pos,
                None => return None,
            };
            if header_pos > 0 {
                self.recv_buffer.drain(..header_pos);
            }
            const MIN_MESSAGE_SIZE: usize = 84;
            if self.recv_buffer.len() < MIN_MESSAGE_SIZE {
                return None;
            }
            let slice = &self.recv_buffer[..MIN_MESSAGE_SIZE];

            if Self::find_subsequence(slice, &Self::SENSOR).is_none() {
                self.recv_buffer.drain(..Self::HEADER.len());
                continue;
            }

            match Self::try_decode_imu(slice) {
                Ok(imu) => {
                    self.recv_buffer.drain(..MIN_MESSAGE_SIZE);
                    return Some(imu);
                }
                Err(_) => {
                    self.recv_buffer.drain(..Self::HEADER.len());
                }
            }
        }
    }

    fn try_decode_imu(data: &[u8]) -> Result<IMUData, std::io::Error> {
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

        Ok(IMUData {
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
pub struct XOImu {
    pub gyro: [f32; 3],
    pub accel: [f32; 3],
    pub timestamp: u64,
}

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
        Ok(imu) => {
            let xo = XOImu {
                gyro: imu.gyro,
                accel: imu.accel,
                timestamp: imu.timestamp,
            };
            unsafe { *out = xo; }
            0
        }
        Err(_) => 1,
    }
}
