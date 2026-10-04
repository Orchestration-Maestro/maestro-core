/* Synthetic injection canary: no downloaded or private code. */
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
__attribute__((constructor)) static void planted_library(void) {
    int descriptor = open("/work/library-executed", O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
    if (descriptor >= 0) close(descriptor);
}
