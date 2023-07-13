use many_error::{define_application_many_error, define_attribute_many_error};

define_attribute_many_error!(
    attribute 16 => {
        2: pub fn invalid_initial_hash(expected, actual)
            => "Invalid initial hash. Expected '{expected}', was '{actual}'.",
    }
);

define_application_many_error!(
    {
        1: pub fn storage_apply_failed(desc) => "Unable to apply change to persistent storage: {desc}.",
        2: pub fn storage_get_failed(desc) => "Unable to get data from persistent storage: {desc}."
    }
);
