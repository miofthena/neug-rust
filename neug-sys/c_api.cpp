#include "c_api.h"
#include "neug/main/neug_db.h"
#include "neug/main/connection.h"
#include <string>
#include <thread>
#include <mutex>
#include <iostream>
#include <cstdlib>
#include <glog/logging.h>

// Thread-local storage for error messages
thread_local std::string g_last_error;

extern "C" {

void neug_init(void) {
    setenv("GLOG_minloglevel", "3", 1);
    setenv("GLOG_stderrthreshold", "3", 1);
}

const char* neug_get_last_error() {
    return g_last_error.c_str();
}

static void set_error(const std::string& msg) {
    g_last_error = msg;
}

neug_db_t neug_db_open(const neug_db_options_t* options) {
    try {
        std::string path = options->db_path ? options->db_path : "";
        std::string mode_str = options->mode ? options->mode : "read-write";
        neug::DBMode mode = (mode_str == "read-only" || mode_str == "r") ? neug::DBMode::READ_ONLY : neug::DBMode::READ_WRITE;
        size_t threads = options->max_thread_num > 0 ? options->max_thread_num : 0;
        
        auto* db = new neug::NeugDB();
        bool success = db->Open(path, threads, mode, "gopt", false, true, true, options->checkpoint_on_close);
        if (!success) {
             set_error("Failed to open NeugDB at path: " + path);
             delete db;
             return nullptr;
        }
        return static_cast<neug_db_t>(db);
    } catch (const std::exception& e) {
        set_error(e.what());
        return nullptr;
    } catch (...) {
        set_error("Unknown error opening database");
        return nullptr;
    }
}

void neug_db_close(neug_db_t db) {
    if (db) {
        auto* neug_db = static_cast<neug::NeugDB*>(db);
        neug_db->Close();
        delete neug_db;
    }
}

neug_conn_t neug_db_connect(neug_db_t db) {
    if (!db) {
        set_error("Invalid database handle");
        return nullptr;
    }
    try {
        auto* neug_db = static_cast<neug::NeugDB*>(db);
        auto conn_ptr = neug_db->Connect();
        if (!conn_ptr) {
            set_error("Failed to establish connection");
            return nullptr;
        }
        // connection_ptr is a std::shared_ptr, we need to allocate a copy of it
        auto* allocated_ptr = new std::shared_ptr<neug::Connection>(conn_ptr);
        return static_cast<neug_conn_t>(allocated_ptr);
    } catch (const std::exception& e) {
        set_error(e.what());
        return nullptr;
    }
}

void neug_conn_close(neug_db_t db, neug_conn_t conn) {
    if (conn) {
        auto* conn_ptr = static_cast<std::shared_ptr<neug::Connection>*>(conn);
        if (db && *conn_ptr) {
             auto* neug_db = static_cast<neug::NeugDB*>(db);
             neug_db->RemoveConnection(*conn_ptr);
        }
        delete conn_ptr; // Decrements ref count
    }
}

// Wrapper struct for QueryResult
struct neug_result_wrapper {
    std::unique_ptr<neug::QueryResult> result;
    std::string error_msg;
    std::string result_str;
    bool is_ok;
};

neug_result_t neug_conn_execute(neug_conn_t conn, const char* query, const char* access_mode) {
    if (!conn || !query) {
        set_error("Invalid connection or query");
        return nullptr;
    }
    
    auto* wrapper = new neug_result_wrapper();
    try {
        auto* conn_ptr = static_cast<std::shared_ptr<neug::Connection>*>(conn);
        std::string query_str(query);
        std::string mode_str = access_mode ? access_mode : "";
        
        auto res = (*conn_ptr)->Query(query_str, mode_str);
        if (res.has_value()) {
             wrapper->is_ok = true;
             wrapper->result = std::make_unique<neug::QueryResult>(std::move(res.value()));
        } else {
             wrapper->is_ok = false;
             wrapper->error_msg = res.error().error_message();
        }
        return static_cast<neug_result_t>(wrapper);
    } catch (const std::exception& e) {
        wrapper->is_ok = false;
        wrapper->error_msg = e.what();
        return static_cast<neug_result_t>(wrapper);
    } catch (...) {
        wrapper->is_ok = false;
        wrapper->error_msg = "Unknown execution exception";
        return static_cast<neug_result_t>(wrapper);
    }
}

void neug_result_free(neug_result_t result) {
    if (result) {
        delete static_cast<neug_result_wrapper*>(result);
    }
}

bool neug_result_is_ok(neug_result_t result) {
    if (!result) return false;
    auto* wrapper = static_cast<neug_result_wrapper*>(result);
    return wrapper->is_ok;
}

const char* neug_result_get_error(neug_result_t result) {
    if (!result) return "Invalid result handle";
    auto* wrapper = static_cast<neug_result_wrapper*>(result);
    return wrapper->error_msg.c_str();
}

const char* neug_result_to_string(neug_result_t result) {
    if (!result) return "";
    auto* wrapper = static_cast<neug_result_wrapper*>(result);
    if (wrapper->is_ok && wrapper->result) {
        wrapper->result_str = wrapper->result->ToString();
        return wrapper->result_str.c_str();
    }
    return "";
}

} // extern "C"
