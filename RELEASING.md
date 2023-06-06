# Releasing `many-rs`
This document provides instructions on how to release a new version on `many-rs`.

1. Clean your repository from dirty files.
    ```shell
    # Dry-run. Respects .gitignore.
    $ git clean -fdn
   
    # Remove the files
    $ git clean -fd
    ```
1. Create the new release
    ```shell
    # Replace [VERSION] with a semver-compatible version
    $ ./release v[VERSION]
    
    # Review the changes
   
    # Push the new master
    $ git push
   
    # Push the tags
    $ git push --tags
    ```

The process will automatically update the `CHANGELOG.md` file with the latest changes.
Pushing the tag will create a new release on the GitHub release page.