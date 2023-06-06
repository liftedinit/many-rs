Feature: Remove token extended info

@tokens
Scenario: Remove token memo extended info as myself
	Given a default token owned by myself
	And the token has a memo
	And an event memo "Fun"
	When I remove the memo as myself
	Then the token has no memo
	And the event memo is "Fun"

@tokens
Scenario: Remove token logo extended info as myself
	Given a default token owned by myself
	And the token has a logo
	When I remove the logo as myself
	Then the token has no logo

@tokens
Scenario: Remove token memo and logo extended info as myself
	Given a default token owned by myself
	And the token has a memo
	And the token has a logo
	When I remove the memo as myself
	And I remove the logo as myself
	Then the token has no memo
	And the token has no logo

@tokens
Scenario: Remove ext. info to a token owned by myself as anonymous/random
	Given a default token owned by myself
	And the token has a memo
	Then removing extended info memo as anonymous fails with unauthorized
	Then removing extended info memo as random fails with unauthorized

@tokens
Scenario: Remove ext. info to a token owned by anonymous
	Given a default token owned by anonymous
	And the token has a memo
	Then removing extended info memo as myself fails with unauthorized

@tokens
Scenario: Remove ext. info to a token owned by random
	Given a default token owned by random
	And the token has a memo
	Then removing extended info memo as myself fails with unauthorized

@tokens
Scenario: Remove ext. info to a token owned by no one
	Given a default token owned by no one
	And the token has a memo
	Then removing extended info memo as myself fails with immutable
	Then removing extended info memo as anonymous fails with immutable
	Then removing extended info memo as random fails with immutable

@tokens
Scenario: Remove ext. info to a token, sender is myself, token owner is account I'm not part of
	Given a token account
	And id 5 as the account owner
	And a default token owned by the account
	And the token has a memo
	Then removing extended info memo as myself fails with missing permission
	And the error role is canTokensRemoveExtendedInfo

@tokens
Scenario: Remove ext. info to a token, sender is myself, token owner is account I'm the owner of
	Given a token account
	And myself as the account owner
	And a default token owned by the account
	And the token has a memo
	When I remove the memo as myself
	Then the token has no memo

@tokens
Scenario: Remove ext. info to a token, sender is some id, token owner is account where some id is part of and with token remove extended info permission
	Given a token account
	And id 5 has token remove extended info permission
	And a default token owned by the account
	And the token has a logo
	When I remove the logo as id 5
	Then the token has no logo

@tokens
Scenario: Remove ext. info to a token, sender is some id, token owner is account where some id is part of without token remove extended info permission
	Given a token account
	And id 6 has token mint permission
	And a default token owned by the account
	And the token has a logo
	Then removing extended info logo as id 6 fails with missing permission
	And the error role is canTokensRemoveExtendedInfo
