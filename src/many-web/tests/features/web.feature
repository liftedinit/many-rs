Feature: dWeb

@web
Scenario: Deploy
  Given a GitHub source with repo url "https://github.com/fmorency/test_dweb.git"
  And a website name "test_dweb"
  And a website description "This is a test"
  Then the website is deployed
