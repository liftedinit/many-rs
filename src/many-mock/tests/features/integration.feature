Feature: Mock Server

Scenario: The server should answer with a JSON
  Given I request "object"
  Then "numfield" should be 10
  Then "arrayfield" should be ["foo", "bar", "baz"]

Scenario: The server should answer with a string
  Given I request "simplefield"
  Then it should be "hello"
