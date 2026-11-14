#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <spawn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
extern char **environ;
static const char *root = "/data/user/0/com.pocketide.pocketterm/files/";
#if defined(__aarch64__) || defined(__x86_64__)
static const char *linker = "/system/bin/linker64";
#else
static const char *linker = "/system/bin/linker";
#endif
static int in_root(const char *p) { return p && strncmp(p, root, strlen(root)) == 0; }
static int is_elf(const char *p) { unsigned char h[4]; int fd=open(p,O_RDONLY|O_CLOEXEC); if(fd<0)return 0; ssize_t n=read(fd,h,4); close(fd); return n==4&&h[0]==0x7f&&h[1]=='E'&&h[2]=='L'&&h[3]=='F'; }
static char *script_interpreter(const char *p) { static char line[4096]; int fd=open(p,O_RDONLY|O_CLOEXEC); if(fd<0)return NULL; ssize_t n=read(fd,line,sizeof(line)-1); close(fd); if(n<3||line[0]!='#'||line[1]!='!')return NULL; line[n]=0; char *s=line+2; while(*s==' '||*s=='\t')++s; char *e=s; while(*e&&*e!='\n'&&*e!=' '&&*e!='\t'&&*e!='\r')++e; if(e==s)return NULL; *e=0; return s; }
static int rewrite(const char *p) { return in_root(p)&&(is_elf(p)||script_interpreter(p)); }
static char **linker_argv(const char *p,char *const av[]) { size_t n=0; while(av&&av[n])++n; char **out=calloc(n+3,sizeof(char*)); if(!out)return NULL; out[0]=(char*)linker; char *interp=script_interpreter(p); if(interp){out[1]=interp;out[2]=(char*)p;for(size_t i=1;i<n;++i)out[i+2]=av[i];}else{out[1]=(char*)p;for(size_t i=1;i<n;++i)out[i+1]=av[i];}return out; }
int execve(const char *p,char *const av[],char *const ep[]) { static int(*real)(const char*,char *const[],char *const[]); if(!real)real=dlsym(RTLD_NEXT,"execve"); if(!rewrite(p))return real(p,av,ep); char **args=linker_argv(p,av); if(!args){errno=ENOMEM;return -1;} real(args[0],args,ep); int e=errno;free(args);errno=e;return -1; }
int execv(const char *p,char *const av[]) { return execve(p,av,environ); }
int execvp(const char *p,char *const av[]) { if(in_root(p))return execve(p,av,environ); static int(*real)(const char*,char *const[]);if(!real)real=dlsym(RTLD_NEXT,"execvp");return real(p,av); }
int posix_spawn(pid_t *pid,const char *p,const posix_spawn_file_actions_t *fa,const posix_spawnattr_t *at,char *const av[],char *const ep[]) { static int(*real)(pid_t*,const char*,const posix_spawn_file_actions_t*,const posix_spawnattr_t*,char *const[],char *const[]);if(!real)real=dlsym(RTLD_NEXT,"posix_spawn");if(!rewrite(p))return real(pid,p,fa,at,av,ep);char **args=linker_argv(p,av);if(!args)return ENOMEM;int rc=real(pid,args[0],fa,at,args,ep);free(args);return rc; }
