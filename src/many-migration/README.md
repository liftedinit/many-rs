# MANY Migration Framework

This folder contains the library for building migrations used in `many-framework`.

Migrations can be one of multiple types;

1. Regular Migration, which contains an initialize function and an update function.
2. Hotfix migrations, which are meant to transform data store values at a single point (block height and key).

## Regular Migrations

Regular migrations initialize at a certain point, executing code with a mutable version of the data store.
This can be used to migrate the data forward to a new format.
After this initialization, the migration is considered active and also executes an update function with every block.

Code can either apply update steps with every blocks, or verify the status of a migration before forking the execution to new behaviour.

Assuming a migration that should activate at block 5, here's what the timeline would look like:

```text

Blocks              1  2  3  4  5  6  7  8  9  ...
                    |  |  |  |  |  |  |  |  |
Migration           |  |  |  |  |  |  |  |  |
    is_active       |  |  |  |  |------------
    initialize()                *
    update()                       *  *  *  *

```
