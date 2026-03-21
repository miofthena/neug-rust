#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdbool.h>
#include <stddef.h>

// Opaque handles
typedef void* neug_db_t;
typedef void* neug_conn_t;
typedef void* neug_result_t;

// Database Options
typedef struct {
    const char* db_path;
    const char* mode; // "read-only" or "read-write"
    size_t max_thread_num;
    bool checkpoint_on_close;
} neug_db_options_t;

// Error handling
// Many functions return a boolean indicating success.
// If false is returned, neug_get_last_error() can be called to get the message.
const char* neug_get_last_error(void);

// Global Initialization
// Should be called once before any other operations to initialize logging and environment.
void neug_init(void);

// Database Operations
neug_db_t neug_db_open(const neug_db_options_t* options);
void neug_db_close(neug_db_t db);

// Connection Operations
neug_conn_t neug_db_connect(neug_db_t db);
void neug_conn_close(neug_db_t db, neug_conn_t conn);

// Query Execution
// Executes a query and returns a result handle. Returns NULL on failure.
neug_result_t neug_conn_execute(neug_conn_t conn, const char* query, const char* access_mode);

// Result Operations
void neug_result_free(neug_result_t result);
bool neug_result_is_ok(neug_result_t result);
const char* neug_result_get_error(neug_result_t result);
// Future: Add methods to iterate over records in the result.

#ifdef __cplusplus
}
#endif
