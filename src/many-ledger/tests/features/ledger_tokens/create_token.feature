Feature: Create ledger Tokens

@tokens
Scenario: Creating a new token as myself
	Given a ticker FBR
	And a name Foobar
	And a decimals of 9
	And a memo "Fun"
	And myself as owner
	Given id 1 has 123 initial tokens
	And id 2 has 456 initial tokens
	When the token is created as myself
	Then the token symbol is a subresource
	And the token ticker is FBR
	And the token name is Foobar
	And the token owner is myself
	And the token total supply is 579
	And the token circulating supply is 579
	And the token maximum supply has no maximum
	And the memo is "Fun"

@tokens
Scenario: Creating a new token, sender is myself, token owner is random
	Given random as owner
	Then creating the token as myself fails with unauthorized

@tokens
Scenario: Creating a new token, sender is myself, token owner is anonymous
	Given anonymous as owner
	Then creating the token as myself fails with unauthorized

@tokens
Scenario: Creating a new token, sender is anon/random, token owner is myself
	Given myself as owner
	Then creating the token as anonymous fails with unauthorized
	Then creating the token as random fails with unauthorized

@tokens
Scenario: Creating a new token, sender is myself, token owner is account I'm not part of
	Given a token account
	And id 5 as the account owner
	And setting the account as the owner
	Then creating the token as myself fails with missing permission
	And the error role is canTokensCreate

@tokens
Scenario: Creating a new token, sender is myself, token owner is account I'm the owner of
	Given a token account
	And myself as the account owner
	And setting the account as the owner
	When the token is created as myself
	Then the token owner is the account

@tokens
Scenario: Creating a new token, sender is some id, token owner is account where some id is part of and with token creation permission
	Given a token account
	And id 5 has token creation permission
	And setting the account as the owner
	When the token is created as id 5
	Then the token owner is the account

@tokens
Scenario: Creating a new token, sender is some id, token owner is account where some id is part of without token creation permission
	Given a token account
	And id 6 has token mint permission
	And setting the account as the owner
	Then creating the token as id 6 fails with missing permission
	And the error role is canTokensCreate

@tokens
Scenario: Creating a new token without owner (owner is sender)
	Given a ticker FOO
	And no owner
	When the token is created as myself
	Then the token ticker is FOO
	And the token owner is myself

@tokens
Scenario: Creating a new token, removing the owner
	Given a ticker FOO
	And removing the owner
	When the token is created as myself
	Then the token ticker is FOO
	And the owner is removed
