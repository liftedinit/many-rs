Feature: Burn token

@tokens
Scenario: Burn tokens
	Given a default token owned by myself
	And a distribution of 12 tokens to id 2
	And a distribution of 23 tokens to id 3
	And a memo "Barfoo"
	When I burn the tokens as token identity
	Then id 1 has 123 tokens
	And id 2 has 444 tokens
	And id 3 has 766 tokens
	And the circulating supply is 1333 tokens
	And the total supply is 1333 tokens
	And the memo is "Barfoo"

@tokens
Scenario: Burn tokens as myself/random/anonymous
	Given a default token owned by myself
	And a distribution of 12 tokens to id 2
	And a distribution of 23 tokens to id 3
	Then burning as myself fails with invalid sender
	Then burning as random fails with invalid sender
	Then burning as anonymous fails with invalid sender
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Unable to burn zero
	Given a default token owned by myself
	And a distribution of 0 tokens to id 2
	Then burning as token identity fails with unable to distribute zero
	And the error address is id 2
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Partial burn is disabled
	Given a default token owned by myself
	And a distribution of 1234 tokens to id 2
	And partial burn is enabled
	Then burning as token identity fails with partial burn disabled
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Burn more funds than held
	Given a default token owned by myself
	And a distribution of 1234 tokens to id 2
	Then burning as token identity fails with missing funds
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Burn all circulating supply
	Given a default token owned by myself
	And a distribution of 123 tokens to id 1
	And a distribution of 456 tokens to id 2
	And a distribution of 789 tokens to id 3
	When I burn the tokens as token identity
	Then id 1 has 0 tokens
	And id 2 has 0 tokens
	And id 3 has 0 tokens
	And the circulating supply is 0 tokens
	And the total supply is 0 tokens

@tokens
Scenario: Burn more than circulating supply
	Given a default token owned by myself
	And a distribution of 124 tokens to id 1
	And a distribution of 456 tokens to id 2
	And a distribution of 789 tokens to id 3
	Then burning as token identity fails with missing funds
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens
