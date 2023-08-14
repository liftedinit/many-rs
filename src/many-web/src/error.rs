use many_error::{define_application_many_error, define_attribute_many_error};

define_attribute_many_error!(
    attribute 16 => {
        1: pub fn invalid_site_name(name) => "Invalid site name: {name}.",
        2: pub fn invalid_initial_hash(expected, actual)
            => "Invalid initial hash. Expected '{expected}', was '{actual}'.",
        3: pub fn invalid_site_description(desc) => "Invalid site description: {desc}.",
        4: pub fn unable_to_create_tempdir(dir) => "Unable to create temporary directory: {dir}.",
        5: pub fn key_not_found(key) => "Key not found: {key}.",
        6: pub fn unable_to_read_entry(entry) => "Unable to read entry: {entry}.",
        7: pub fn key_should_start_with_http() => "Key should start with '/http/'.",
        8: pub fn unable_to_strip_prefix(prefix) => "Unable to strip prefix: {prefix}.",
        9: pub fn unable_to_convert_to_str() => "Unable to convert to str.",
        10: pub fn io_error(err) => "I/O error: {err}.",
        11: pub fn invalid_zip_file(err) => "Invalid zip file: {err}.",
        12: pub fn unable_to_extract_zip_file(err) => "Unable to extract zip file: {err}.",
        13: pub fn invalid_owner(owner) => "Invalid owner: {owner}.",
        14: pub fn unable_to_open_storage(err) => "Unable to open storage: {err}.",
    }
);

define_application_many_error!(
    {
        1: pub fn storage_apply_failed(desc) => "Unable to apply change to persistent storage: {desc}.",
        2: pub fn storage_get_failed(desc) => "Unable to get data from persistent storage: {desc}.",
        3: pub fn storage_commit_failed(desc) => "Unable to commit data to persistent storage: {desc}.",
    }
);
