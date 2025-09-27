#ifndef XREAL_ONE_DRIVER_H
#define XREAL_ONE_DRIVER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct XrealOneHandle XrealOneHandle;

typedef struct XOImu {
    float gyro[3];
    float accel[3];
    uint64_t timestamp;
} XOImu;

// Returns NULL on failure
XrealOneHandle* xo_new(void);

// Returns NULL on failure; addr must be a null-terminated string like "169.254.2.1:52998"
XrealOneHandle* xo_new_with_addr(const char* addr);

// 0 on success, non-zero on error; fills *out on success
int xo_next(XrealOneHandle* handle, XOImu* out);

// Safe to call with NULL
void xo_free(XrealOneHandle* handle);

#ifdef __cplusplus
}
#endif

#endif /* XREAL_ONE_DRIVER_H */
