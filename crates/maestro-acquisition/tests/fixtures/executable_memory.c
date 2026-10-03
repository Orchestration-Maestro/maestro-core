/* Independently authored synthetic W^X/bypass probe, compiled and pinned offline. */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/personality.h>
#include <sys/syscall.h>
#include <unistd.h>

static int denied_map(int protection) {
    errno = 0;
    void *memory = mmap(NULL, 4096, protection, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    return memory == MAP_FAILED && errno == EPERM;
}
static int denied_call(long number) {
    errno = 0;
    return syscall(number, 0, 0, 0, 0, 0, 0) == -1 && errno == EPERM;
}
int main(void) {
    int safe = denied_map(PROT_READ | PROT_WRITE | PROT_EXEC) && denied_map(PROT_READ | PROT_EXEC);
    void *memory = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (memory == MAP_FAILED) return 2;
    errno = 0;
    safe &= mprotect(memory, 4096, PROT_READ | PROT_EXEC) == -1 && errno == EPERM;
    munmap(memory, 4096);
    errno = 0;
    safe &= syscall(SYS_memfd_create, "planted", 0) == -1 && errno == EPERM;
    errno = 0;
    safe &= personality(READ_IMPLIES_EXEC) == -1 && errno == EPERM;
    safe &= open("/proc/self/mem", O_WRONLY) == -1;
    safe &= denied_call(SYS_pkey_mprotect) && denied_call(SYS_shmget) && denied_call(SYS_shmat);
    safe &= denied_call(SYS_io_uring_setup) && denied_call(SYS_io_uring_enter) && denied_call(SYS_io_uring_register);
    safe &= denied_call(SYS_userfaultfd) && denied_call(SYS_ptrace) && denied_call(SYS_process_vm_writev);
    const char *result = safe ? "WX_DENIED" : "WX_ESCAPED";
    printf("{\"kind\":\"decode\",\"value\":{\"stage\":\"attachment\",\"input_bytes\":100,\"expanded_bytes\":%zu,\"levels\":1,\"members\":1,\"entities\":0,\"pixels\":0,\"memory_bytes\":%zu}}\n", strlen(result), strlen(result));
    fflush(stdout);
    char ack[16];
    if (!fgets(ack, sizeof ack, stdin) || strcmp(ack, "OK\n") != 0) return 3;
    printf("{\"kind\":\"data\",\"value\":\"%s\"}\n{\"kind\":\"done\"}\n", result);
    return 0;
}
