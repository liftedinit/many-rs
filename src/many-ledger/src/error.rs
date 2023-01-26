use many_error::{define_application_many_error, define_attribute_many_error};

define_attribute_many_error!(
    attribute 2 => {
        1: pub fn unknown_symbol(symbol) => "Symbol not supported by this ledger: {symbol}.",
        2: pub fn unauthorized() => "Unauthorized to do this operation.",
        3: pub fn insufficient_funds() => "Insufficient funds.",
        4: pub fn anonymous_cannot_hold_funds() => "Anonymous is not a valid account identity.",
        5: pub fn invalid_initial_state(expected, actual)
            => "Invalid initial state hash. Expected '{expected}', was '{actual}'.",
        6: pub fn unexpected_subresource_id(expected, actual)
            => "Invalid initial state account subresource_id. Expected '{expected}', was '{actual}'.",
        7: pub fn unexpected_account_id(expected, actual)
            => "Invalid initial state account id. Expected '{expected}', was '{actual}'.",
        8: pub fn destination_is_source()
            => "Unable to send tokens to a destination (to) that is the same as the source (from).",
        9: pub fn amount_is_zero()
            => "Unable to send zero (0) token.",
        10: pub fn storage_key_not_found(key) => "Key not found in storage: {key:?}.",
    }
);

define_attribute_many_error!(
    attribute 11 => {
        1: pub fn token_info_not_found(symbol) => "Token information not found in persistent storage: {symbol}.",
        2: pub fn ext_info_not_found(symbol) => "Token extended information not found in persistent storage: {symbol}.",
        3: pub fn invalid_sender() => "Unauthorised Token endpoints sender.",
    }
);

define_attribute_many_error!(
    attribute 12 => {
        1: pub fn symbol_not_found(symbol) => "Unable to mint/burn a unknown symbol: {symbol}.",
        2: pub fn over_maximum_supply(symbol, amount, max) => "Unable to mint over the maximum symbol supply : {amount} > {max} {symbol}.",
        3: pub fn missing_funds(symbol, amount, balance) => "Unable to burn, missing funds: {amount} > {balance} {symbol}.",
        4: pub fn unable_to_distribute_zero(symbol) => "The mint/burn distribution contains zero for {symbol}.",
        5: pub fn partial_burn_disabled() => "Partial burns are disabled.",
    }
);

define_application_many_error!(
    {
        1: pub fn storage_apply_failed(desc) => "Unable to apply change to persistent storage: {desc}.",
        2: pub fn storage_get_failed(desc) => "Unable to get data from persistent storage: {desc}.",
        3: pub fn storage_commit_failed(desc) => "Unable to commit data to persistent storage: {desc}.",
        4: pub fn storage_open_failed(desc) => "Unable to open persistent storage: {desc}.",
        5: pub fn unable_to_load_migrations(desc) => "Unable to load migrations: {desc}.",
    }
);
