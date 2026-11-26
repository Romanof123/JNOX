#include <jnox/process.h>

enum {
    BOOT_IMAGE_BYTES = 8192,
    BOOT_ENTRY_START = 16, BOOT_ENTRY_BYTES = 48, BOOT_ENTRIES = 8,
    BOOT_DATA_START = BOOT_ENTRY_START + BOOT_ENTRIES * BOOT_ENTRY_BYTES,
    BOOT_NAME_BYTES = 32, BOOT_FD_BASE = 16, BOOT_FD_COUNT = 4
};
static const volatile unsigned char *const boot_image =
    (const volatile unsigned char *)0x01800000UL;
static struct boot_description {
    unsigned offset, length, position, open;
} boot_files[BOOT_FD_COUNT];

static uint32_t boot_le32(unsigned at) {
    return (uint32_t)boot_image[at] | (uint32_t)boot_image[at + 1] << 8
         | (uint32_t)boot_image[at + 2] << 16 | (uint32_t)boot_image[at + 3] << 24;
}
static int boot_fd(int fd) {
    return fd >= BOOT_FD_BASE && fd < BOOT_FD_BASE + BOOT_FD_COUNT;
}

static long boot_file_open(const char *path, int flags) {
    static const char magic[] = "JNOXROM1";
    long n = fifo_name_length(path);
    if (n < 0) return n;
    for (unsigned i = 0; i < sizeof magic - 1; ++i)
        if (boot_image[i] != (unsigned char)magic[i]) return -EINVAL;
    unsigned count = boot_le32(8);
    if (count > BOOT_ENTRIES) return -EINVAL;

    unsigned selected_offset = 0, selected_length = 0;
    for (unsigned i = 0; i < count; ++i) {
        unsigned entry = BOOT_ENTRY_START + i * BOOT_ENTRY_BYTES;
        unsigned name_len = 0;
        int matches = 1;
        while (name_len < BOOT_NAME_BYTES && boot_image[entry + name_len]) {
            if (name_len >= (unsigned)n ||
                boot_image[entry + name_len] != (unsigned char)path[name_len]) matches = 0;
            ++name_len;
        }
        unsigned offset = boot_le32(entry + 32), length = boot_le32(entry + 36);
        if (!name_len || name_len == BOOT_NAME_BYTES ||
            offset < BOOT_DATA_START || offset > BOOT_IMAGE_BYTES ||
            length > BOOT_IMAGE_BYTES - offset) return -EINVAL;
        if (matches && name_len == (unsigned)n) {
            if (selected_offset) return -EINVAL; /* ambiguous catalog name */
            selected_offset = offset;
            selected_length = length;
        }
    }
    if (!selected_offset) return -ENOENT;
    if (flags != O_RDONLY) return -EACCES;
    for (unsigned i = 0; i < BOOT_FD_COUNT; ++i) {
        if (!boot_files[i].open) {
            boot_files[i] = (struct boot_description){selected_offset, selected_length, 0, 1};
            return BOOT_FD_BASE + i;
        }
    }
    return -EMFILE;
}

static long boot_file_read(int fd, void *buffer, size_t count) {
    struct boot_description *file = &boot_files[fd - BOOT_FD_BASE];
    if (!file->open) return -EBADF;
    unsigned left = file->length - file->position;
    unsigned n = count < left ? (unsigned)count : left;
    if (n && !buffer) return -EFAULT;
    for (unsigned i = 0; i < n; ++i)
        ((unsigned char *)buffer)[i] = boot_image[file->offset + file->position + i];
    file->position += n;
    return n;
}

static long boot_file_close(int fd) {
    struct boot_description *file = &boot_files[fd - BOOT_FD_BASE];
    if (!file->open) return -EBADF;
    file->open = 0;
    return 0;
}

int jnox_spawn(const char *program, char *const argv[], int stdin_fd, int stdout_fd) {
    return (int)pipe_result(raw_service(50, (long)program, (long)argv, stdin_fd, stdout_fd));
}

int jnox_waitpid(int pid, int *status) {
    struct service_result r = raw_service_pair(51, pid, 0, 0, 0, 0);
    if (r.status < 0) return (int)pipe_result(r.status);
    if (status) *status = (int)(long)r.value;
    return pid;
}

int jnox_dmesg(char *buffer, size_t length) {
    return (int)service(70, (long)buffer, (long)length, 0);
}

int jnox_shell_getchar(void) {
    return (int)service(71, 0, 0, 0);
}
