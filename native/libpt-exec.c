#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <spawn.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

extern char **environ;

typedef int (*execve_fn)(const char *, char *const[], char *const[]);
typedef int (*execvp_fn)(const char *, char *const[]);
typedef int (*spawn_fn)(pid_t *, const char *, const posix_spawn_file_actions_t *,
                        const posix_spawnattr_t *, char *const[], char *const[]);
typedef int (*spawnp_fn)(pid_t *, const char *, const posix_spawn_file_actions_t *,
                         const posix_spawnattr_t *, char *const[], char *const[]);

static const char *linker_path(void) {
#if defined(__aarch64__)
    return "/system/bin/linker64";
#else
    return "/system/bin/linker";
#endif
}

static int in_private_root(const char *path) {
    const char *root = getenv("POCKETTERM_FILES_DIR");
    if (path == NULL || root == NULL || root[0] != '/') return 0;
    char canonical_root[PATH_MAX];
    char canonical_path[PATH_MAX];
    if (realpath(root, canonical_root) == NULL ||
        realpath(path, canonical_path) == NULL) return 0;
    size_t length = strlen(canonical_root);
    while (length > 1 && canonical_root[length - 1] == '/') --length;
    return strncmp(canonical_path, canonical_root, length) == 0 &&
           (canonical_path[length] == '/' || canonical_path[length] == '\0');
}

static int is_elf(const char *path) {
    unsigned char header[24];
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return 0;
    ssize_t count = read(fd, header, sizeof(header));
    close(fd);
    return count >= (ssize_t)sizeof(header) && header[0] == 0x7f &&
           header[1] == 'E' && header[2] == 'L' && header[3] == 'F' &&
           header[4] == 2 && header[5] == 1 && header[6] == 1 &&
           (header[16] == 2 || header[16] == 3) && header[17] == 0 &&
           header[18] == 183 && header[19] == 0 && header[20] == 1 &&
           header[21] == 0 && header[22] == 0 && header[23] == 0;
}

static int interpreter_for_script(const char *path, char *interpreter,
                                  size_t interpreter_size, char *option,
                                  size_t option_size) {
    char line[4096];
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return 0;
    ssize_t count = read(fd, line, sizeof(line) - 1);
    close(fd);
    if (count < 3 || line[0] != '#' || line[1] != '!') return 0;
    line[count] = '\0';
    char *value = line + 2;
    while (*value == ' ' || *value == '\t') ++value;
    char *end = value;
    while (*end != '\0' && *end != '\n' && *end != '\r' && *end != ' ' && *end != '\t') ++end;
    if (end == value || (size_t)(end - value) >= interpreter_size) return 0;
    memcpy(interpreter, value, (size_t)(end - value));
    interpreter[end - value] = '\0';
    while (*end == ' ' || *end == '\t') ++end;
    char *option_end = end;
    while (*option_end != '\0' && *option_end != '\n' && *option_end != '\r') ++option_end;
    size_t option_length = (size_t)(option_end - end);
    if (option_length >= option_size) option_length = option_size - 1;
    memcpy(option, end, option_length);
    option[option_length] = '\0';
    return 1;
}

static int should_rewrite(const char *path) {
    if (!in_private_root(path) || access(path, X_OK) != 0) return 0;
    if (is_elf(path)) return 1;
    char interpreter[PATH_MAX];
    char option[4096];
    return interpreter_for_script(path, interpreter, sizeof(interpreter),
                                  option, sizeof(option));
}

static char **linker_arguments(const char *path, char *const arguments[],
                              size_t *allocated_count) {
    size_t count = 0;
    while (arguments != NULL && arguments[count] != NULL) ++count;
    size_t argument_count = count == 0 ? 1 : count;
    char interpreter[PATH_MAX];
    char option[4096];
    int script = !is_elf(path) && interpreter_for_script(
        path, interpreter, sizeof(interpreter), option, sizeof(option));
    if (!is_elf(path) && !script) {
        errno = ENOEXEC;
        return NULL;
    }
    int has_option = script && option[0] != '\0';
    size_t extra = script ? (has_option ? 4 : 3) : 2;
    char **result = calloc(argument_count + extra, sizeof(char *));
    if (result == NULL) return NULL;
    size_t at = 0;
    result[at++] = (char *)linker_path();
    if (script) {
        result[at++] = strdup(interpreter);
        if (has_option) result[at++] = strdup(option);
        result[at++] = strdup(path);
    } else {
        result[at++] = strdup(path);
    }
    for (size_t index = 1; index < count; ++index) result[at++] = strdup(arguments[index]);
    for (size_t index = 1; index < at; ++index) {
        if (result[index] == NULL) {
            for (size_t allocated = 1; allocated < at; ++allocated) free(result[allocated]);
            free(result);
            errno = ENOMEM;
            return NULL;
        }
    }
    result[at] = NULL;
    *allocated_count = at;
    return result;
}

static void release_arguments(char **arguments, size_t count) {
    if (arguments == NULL) return;
    for (size_t index = 1; index < count; ++index) free(arguments[index]);
    free(arguments);
}

static char *private_path_candidate(const char *name) {
    if (name == NULL || strchr(name, '/') != NULL) return NULL;
    const char *path = getenv("PATH");
    if (path == NULL) return NULL;
    const char *cursor = path;
    while (*cursor != '\0') {
        const char *end = strchr(cursor, ':');
        size_t length = end == NULL ? strlen(cursor) : (size_t)(end - cursor);
        if (length > 0 && length + strlen(name) + 2 < PATH_MAX) {
            char candidate[PATH_MAX];
            memcpy(candidate, cursor, length);
            candidate[length] = '/';
            strcpy(candidate + length + 1, name);
            if (in_private_root(candidate) && access(candidate, X_OK) == 0 && should_rewrite(candidate))
                return strdup(candidate);
        }
        if (end == NULL) break;
        cursor = end + 1;
    }
    return NULL;
}

int execve(const char *path, char *const arguments[], char *const environment[]) {
    static execve_fn real_execve;
    if (real_execve == NULL) real_execve = (execve_fn)dlsym(RTLD_NEXT, "execve");
    if (real_execve == NULL) { errno = ENOSYS; return -1; }
    if (!should_rewrite(path)) return real_execve(path, arguments, environment);
    size_t count = 0;
    char **rewritten = linker_arguments(path, arguments, &count);
    if (rewritten == NULL) { if (errno == 0) errno = ENOMEM; return -1; }
    real_execve(rewritten[0], rewritten, environment);
    int saved_errno = errno;
    release_arguments(rewritten, count);
    errno = saved_errno;
    return -1;
}

int execv(const char *path, char *const arguments[]) {
    return execve(path, arguments, environ);
}

int execvp(const char *path, char *const arguments[]) {
    if (should_rewrite(path)) return execve(path, arguments, environ);
    char *candidate = private_path_candidate(path);
    if (candidate != NULL) {
        int result = execve(candidate, arguments, environ);
        int saved_errno = errno;
        free(candidate);
        errno = saved_errno;
        return result;
    }
    static execvp_fn real_execvp;
    if (real_execvp == NULL) real_execvp = (execvp_fn)dlsym(RTLD_NEXT, "execvp");
    if (real_execvp == NULL) { errno = ENOSYS; return -1; }
    return real_execvp(path, arguments);
}

int posix_spawn(pid_t *pid, const char *path,
                const posix_spawn_file_actions_t *actions,
                const posix_spawnattr_t *attributes, char *const arguments[],
                char *const environment[]) {
    static spawn_fn real_spawn;
    if (real_spawn == NULL) real_spawn = (spawn_fn)dlsym(RTLD_NEXT, "posix_spawn");
    if (real_spawn == NULL) return ENOSYS;
    if (!should_rewrite(path)) return real_spawn(pid, path, actions, attributes, arguments, environment);
    size_t count = 0;
    char **rewritten = linker_arguments(path, arguments, &count);
    if (rewritten == NULL) return errno == 0 ? ENOMEM : errno;
    int result = real_spawn(pid, rewritten[0], actions, attributes, rewritten, environment);
    release_arguments(rewritten, count);
    return result;
}

int posix_spawnp(pid_t *pid, const char *path,
                 const posix_spawn_file_actions_t *actions,
                 const posix_spawnattr_t *attributes, char *const arguments[],
                 char *const environment[]) {
    char *candidate = private_path_candidate(path);
    if (candidate == NULL) {
        static spawnp_fn real_spawnp;
        if (real_spawnp == NULL) real_spawnp = (spawnp_fn)dlsym(RTLD_NEXT, "posix_spawnp");
        return real_spawnp == NULL ? ENOSYS : real_spawnp(pid, path, actions, attributes, arguments, environment);
    }
    int result = posix_spawn(pid, candidate, actions, attributes, arguments, environment);
    free(candidate);
    return result;
}
