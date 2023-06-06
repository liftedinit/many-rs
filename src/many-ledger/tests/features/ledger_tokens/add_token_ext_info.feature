Feature: Add token extended info

@tokens
Scenario: Add token memo extended info as myself
	Given a default token owned by myself
	And a memo "Oh my god, it's full of stars"
	And an event memo "Fun"
	When I add the extended info to the token as myself
	Then the token has the memo "Oh my god, it's full of stars"
	And the event memo is "Fun"

@tokens
Scenario: Add token unicode char logo extended info as myself
	Given a default token owned by myself
	And an unicode logo ∑
	When I add the extended info to the token as myself
	Then the token has the unicode logo ∑

@tokens
Scenario: Add token image logo extended info as myself
	Given a default token owned by myself
	And a png image logo '010203'
	When I add the extended info to the token as myself
	Then the token has the png image logo '010203'

@tokens
Scenario: Add ext. info to a token owned by myself as anonymous/random
	Given a default token owned by myself
	And a memo "So long, and thanks for all the fish"
	Then adding extended info to the token as anonymous fails with unauthorized
	Then adding extended info to the token as random fails with unauthorized

@tokens
Scenario: Adding ext. info to a token owned by anonymous
	Given a default token owned by anonymous
	And a memo "Something clever"
	Then adding extended info to the token as myself fails with unauthorized

@tokens
Scenario: Adding ext. info to a token owned by random
	Given a default token owned by random
	And a memo "Something more clever"
	Then adding extended info to the token as myself fails with unauthorized

@tokens
Scenario: Adding ext. info to a token owned by no one
	Given a default token owned by no one
	And a memo "No one is owning this token"
	Then adding extended info to the token as myself fails with immutable
	Then adding extended info to the token as anonymous fails with immutable
	Then adding extended info to the token as random fails with immutable

@tokens
Scenario: Adding ext. info to a token, sender is myself, token owner is account I'm not part of
	Given a token account
	And id 5 as the account owner
	And a default token owned by the account
	Then adding extended info to the token as myself fails with missing permission
	And the error role is canTokensAddExtendedInfo

@tokens
Scenario: Adding ext. info to a token, sender is myself, token owner is account I'm the owner of
	Given a token account
	And myself as the account owner
	And a default token owned by the account
	And a memo "I'm your father"
	When I add the extended info to the token as myself
	Then the token has the memo "I'm your father"

@tokens
Scenario: Adding ext. info to a token, sender is some id, token owner is account where some id is part of and with token add extended info permission
	Given a token account
	And id 5 has token add extended info permission
	And a default token owned by the account
	And an unicode logo ∑
	When I add the extended info to the token as id 5
	Then the token has the unicode logo ∑

@tokens
Scenario: Adding ext. info to a token, sender is some id, token owner is account where some id is part of without token add extended info permission
	Given a token account
	And id 6 has token mint permission
	And a default token owned by the account
	Then adding extended info to the token as id 6 fails with missing permission
	And the error role is canTokensAddExtendedInfo
