#include <stdio.h>
#include <stdint.h>
#include <signal.h>
#include <stdbool.h>
#include "../include/xreal_one_driver.h"

static volatile sig_atomic_t keep_running = 1;

static void handle_signal(int sig) {
    (void)sig;
    keep_running = 0;
}

int main(void) {
    // Install Ctrl-C/TERM handler for clean shutdown
    signal(SIGINT, handle_signal);
    signal(SIGTERM, handle_signal);

    XrealOneHandle* h = xo_new();
    if (!h) {
        fprintf(stderr, "xo_new failed (glasses may be disconnected)\n");
        return 0; // Exit gracefully if device not reachable
    }

    XOImu imu;
    while (keep_running) {
        int rc = xo_next(h, &imu);
        if (rc != 0) {
            printf("xo_next rc=%d (stopping)\n", rc);
            break;
        }
        printf("ts=%llu gyro=[%f %f %f] accel=[%f %f %f]\n",
               (unsigned long long)imu.timestamp,
               imu.gyro[0], imu.gyro[1], imu.gyro[2],
               imu.accel[0], imu.accel[1], imu.accel[2]);
        fflush(stdout);
    }

    xo_free(h);
    return 0;
}
