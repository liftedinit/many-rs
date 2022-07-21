use many_error::define_attribute_many_error;

define_attribute_many_error!(
    attribute 9 => {
        1: pub fn unknown_account(id) => "Account with ID {id} unknown.",
        2: pub fn unknown_role(role) => "Account does not know role '{role}'.",
        3: pub fn user_needs_role(role) => "Sender needs role '{role}' to perform this operation.",
        4: pub fn account_must_own_itself() => "Unable to remove owner role from the account itself.",
        5: pub fn empty_feature() => "At least one feature must be selected.",
    }
);
