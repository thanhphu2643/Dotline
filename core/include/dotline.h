#ifndef DOTLINE_H
#define DOTLINE_H
#include <stdint.h>

/* Every returned string is JSON, owned by the caller: free with dotline_string_free. */
char *dotline_start(const char *data_dir, uint16_t port);
void dotline_stop(void);
char *dotline_snapshot(void);
char *dotline_rotate_token(void);
char *dotline_test_print(const char *printer, const char *language);
char *dotline_job_preview(const char *job_id);
char *dotline_file_preview(const char *path);
void dotline_clear_jobs(void);
void dotline_string_free(char *ptr);

#endif
