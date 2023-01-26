Feature: Mint token

@tokens
Scenario: Mint new tokens as token identity
	Given a default token owned by myself
	And a distribution of 1000 tokens to id 10
	And a distribution of 250 tokens to id 11
	And a memo "Foobar"
	When I mint the tokens as token identity
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And id 10 has 1000 tokens
	And id 11 has 250 tokens
	And myself has 0 tokens
	And the circulating supply is 2618 tokens
	And the total supply is 2618 tokens
	And the memo is "Foobar"

@tokens
Scenario: Mint new tokens as myself/random/anonymous
	Given a default token owned by myself
	And a distribution of 1000 tokens to id 10
	And a distribution of 250 tokens to id 11
	Then minting as myself fails with invalid sender
	Then minting as random fails with invalid sender
	Then minting as anonymous fails with invalid sender
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Unable to mint zero
	Given a default token owned by myself
	And a distribution of 0 tokens to id 10
	Then minting as token identity fails with unable to distribute zero
	And the error address is id 10
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Mint more than maximum supply
	Given a default token owned by myself
	And a distribution of 99998633 tokens to id 10
	Then minting as token identity fails with over maximum
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And the circulating supply is 1368 tokens
	And the total supply is 1368 tokens

@tokens
Scenario: Mint the maximum supply
	Given a default token owned by myself
	And a distribution of 99998632 tokens to id 10
	When I mint the tokens as token identity
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And id 10 has 99998632 tokens
	And the circulating supply is 100000000 tokens
	And the total supply is 100000000 tokens

@tokens
Scenario: Unlimited maximum supply
	Given a default token of unlimited supply owned by myself
	And a distribution of 100000000 tokens to id 10
	When I mint the tokens as token identity
	Then id 1 has 123 tokens
	And id 2 has 456 tokens
	And id 3 has 789 tokens
	And id 10 has 100000000 tokens
	And the circulating supply is 100001368 tokens
	And the total supply is 100001368 tokens
