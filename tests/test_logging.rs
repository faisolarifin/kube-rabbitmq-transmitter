use rabbitmq_consumer::{
    config::LoggerConfig,
    providers::StructuredLogger,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_error_file_logging() {
    // Create a temporary directory for test logs
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path().to_str().unwrap();
    
    let logger_config = LoggerConfig {
        dir: log_dir.to_string(),
        file_name: "test-custom-gateway".to_string(),
        max_backups: 0,
        max_size: 10,
        max_age: 90,
        compress: true,
        local_time: true,
    };

    // Initialize logger with file logging
    StructuredLogger::init("error", Some(logger_config.clone())).unwrap();
    
    // Generate an error log
    StructuredLogger::log_error(
        "Test error message for file logging",
        Some("test-unique-id"),
        Some("test-request-id"),
    );
    
    // Wait a moment for the log to be written
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // Check if log file was created
    let log_files: Vec<_> = fs::read_dir(log_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_name().to_str().unwrap_or("").starts_with("test-custom-gateway")
                && entry.file_name().to_str().unwrap_or("").ends_with(".log")
        })
        .collect();
    
    assert!(!log_files.is_empty(), "No log files found");
    
    // Read the log file content
    let log_file = &log_files[0];
    let log_content = fs::read_to_string(log_file.path()).unwrap();
    
    // Verify that our error message is in the log file
    assert!(log_content.contains("Test error message for file logging"));
    assert!(log_content.contains("test-unique-id"));
    assert!(log_content.contains("test-request-id"));
    
    // Verify that local time is being used (Indonesian time should be UTC+7)
    let current_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let expected_filename = format!("test-custom-gateway.{}.log", current_date);
    assert_eq!(log_file.file_name().to_str().unwrap(), expected_filename);
    
    println!("✅ Log file created successfully: {:?}", log_file.file_name());
    println!("✅ Local time configuration working correctly");
    println!("📄 Log content: {}", log_content);
}