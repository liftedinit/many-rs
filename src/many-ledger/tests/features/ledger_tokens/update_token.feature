Feature: Update ledger Tokens

@tokens
Scenario: Updating a token's ticker as myself
	Given a default token owned by myself
	And a new ticker ABC
	When I update the token as myself
	Then the token new ticker is ABC

@tokens
Scenario: Updating a token's ticker as myself, with memo
	Given a default token owned by myself
	And a new ticker ABC
	And a memo "Some memo"
	When I update the token as myself
	Then the token new ticker is ABC
	And the memo is "Some memo"

@tokens
Scenario: Updating a token's name as myself
	Given a default token owned by myself
	And a new name Supercalifragilisticexpialidocious
	When I update the token as myself
	Then the token new name is Supercalifragilisticexpialidocious

@tokens
Scenario: Updating a token's decimal as myself
	Given a default token owned by myself
	And a new decimal 16
	When I update the token as myself
	Then the token new decimal is 16

@tokens
Scenario: Updating a token owned by myself as anonymous/random
	Given a default token owned by myself
	And a new ticker ABC
	Then updating the token as anonymous fails with unauthorized
	Then updating the token as random fails with unauthorized

@tokens
Scenario: Updating a token owned by anonymous
	Given a default token owned by anonymous
	And a new ticker ABC
	Then updating the token as myself fails with unauthorized

@tokens
Scenario: Updating a token owned by random
	Given a default token owned by random
	And a new ticker ABC
	Then updating the token as myself fails with unauthorized

@tokens
Scenario: Updating a token owned by no one
	Given a default token owned by no one
	And a new ticker ABC
	Then updating the token as myself fails with immutable
	Then updating the token as anonymous fails with immutable
	Then updating the token as random fails with immutable

@tokens
Scenario: Updating a token, sender is myself, token owner is account I'm not part of
	Given a token account
	And id 5 as the account owner
	And a default token owned by the account
	Then updating the token as myself fails with missing permission
	And the error role is canTokensUpdate

@tokens
Scenario: Updating a token, sender is myself, token owner is account I'm the owner of
	Given a token account
	And myself as the account owner
	And a default token owned by the account
	When I update the token as myself
	Then the token new owner is the account

@tokens
Scenario: Updating a token, sender is some id, token owner is account where some id is part of and with token update permission
	Given a token account
	And id 5 has token update permission
	And a default token owned by the account
	When I update the token as id 5
	Then the token new owner is the account

@tokens
Scenario: Updating a token, sender is some id, token owner is account where some id is part of without token update permission
	Given a token account
	And id 6 has token mint permission
	And a default token owned by the account
	Then updating the token as id 6 fails with missing permission
	And the error role is canTokensUpdate

@tokens
Scenario: Removing a token's owner
	Given a default token owned by myself
	And removing the token owner
	When I update the token as myself
	Then the token owner is removed
