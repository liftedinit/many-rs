use crate::define_attribute_many_error;

define_attribute_many_error!(
    attribute 10 => {
        1: pub fn existing_entry() => "The entry key already exists in the storage.",
        2: pub fn entry_not_found(entry) => "Storage was unable to find entry: '{entry}'.",
        3: pub fn invalid_address(addr) => "The identity '{addr}' is invalid.",
        4: pub fn invalid_recall_phrase() => "The number of words should be [2, 5].",
    }
);