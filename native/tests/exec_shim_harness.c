#define _GNU_SOURCE
#include <errno.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

static int report_child(void) {
    const char *required[] = {"PREFIX", "POCKETTERM_FILES_DIR", "HOME", "TMPDIR", "PATH", "LD_LIBRARY_PATH", "LD_PRELOAD"};
    for (size_t index = 0; index < sizeof(required) / sizeof(required[0]); ++index) {
        if (getenv(required[index]) == NULL || getenv(required[index])[0] == '\0') {
            fprintf(stderr, "missing environment variable: %s\n", required[index]);
            return 80;
        }
    }
    fputs("gate stdout OK\n", stdout);
    fputs("gate stderr OK\n", stderr);
    return 23;
}

static int exit_status(pid_t child) {
    int status = 0;
    if (waitpid(child, &status, 0) < 0) return 127;
    if (!WIFEXITED(status)) return 128;
    return WEXITSTATUS(status);
}

static int run_execve(const char *path) {
    pid_t child = fork();
    if (child == 0) {
        char *arguments[] = {(char *)path, "--child", NULL};
        execve(path, arguments, environ);
        _exit(126);
    }
    return child < 0 ? 127 : exit_status(child);
}

static int run_execv(const char *path) {
    pid_t child = fork();
    if (child == 0) {
        char *arguments[] = {(char *)path, "--child", NULL};
        execv(path, arguments);
        _exit(126);
    }
    return child < 0 ? 127 : exit_status(child);
}

static int run_execvp(void) {
    pid_t child = fork();
    if (child == 0) {
        char *arguments[] = {"pt-exec-shim-harness", "--child", NULL};
        execvp(arguments[0], arguments);
        _exit(126);
    }
    return child < 0 ? 127 : exit_status(child);
}

static int run_posix_spawn(const char *path) {
    pid_t child = -1;
    char *arguments[] = {(char *)path, "--child", NULL};
    int result = posix_spawn(&child, path, NULL, NULL, arguments, environ);
    if (result != 0) return 127;
    return exit_status(child);
}

static int run_system(const char *path) {
    char command[4096];
    if (snprintf(command, sizeof(command), "'%s' --child", path) >= (int)sizeof(command)) return 127;
    int status = system(command);
    if (status < 0 || !WIFEXITED(status)) return 128;
    return WEXITSTATUS(status);
}

static int run_shebang(const char *path) {
    char command[4096];
    if (snprintf(command, sizeof(command), "'%s'", path) >= (int)sizeof(command)) return 127;
    int status = system(command);
    if (status < 0 || !WIFEXITED(status)) return 128;
    return WEXITSTATUS(status);
}

static int check(const char *method, int status) {
    if (status != 23) {
        fprintf(stderr, "%s failed: child exit %d, expected 23\n", method, status);
        return 1;
    }
    printf("gate %s OK\n", method);
    return 0;
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--child") == 0) return report_child();
    if (argc < 3) {
        fprintf(stderr, "usage: %s /absolute/path/to/self /absolute/path/to/script\n", argv[0]);
        return 2;
    }
    const char *path = argv[1];
    const char *script = argv[2];
    int failures = 0;
    failures += check("execve", run_execve(path));
    failures += check("execv", run_execv(path));
    failures += check("execvp", run_execvp());
    failures += check("posix_spawn", run_posix_spawn(path));
    failures += check("system", run_system(path));
    failures += check("shebang", run_shebang(script) == 0 ? 23 : 127);
    return failures == 0 ? 0 : 1;
}
