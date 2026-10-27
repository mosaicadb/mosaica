# Inserting and committing

Use `insert` to hand a table to something you have declared, naming the columns it reads from. Nothing reaches the database until `commit`, which builds the database and starts its server the first time, and sends what was inserted since on each later call. To see what the next commit would do without sending anything, call `check`.

## Database

::: mosaica._database.Database.insert
    options:
      heading_level: 3

::: mosaica._database.Database.check
    options:
      heading_level: 3

::: mosaica._database.Database.commit
    options:
      heading_level: 3

::: mosaica._inserts.Insert
    options:
      show_bases: false

::: mosaica._reports.Report
    options:
      show_bases: false

::: mosaica._reports.CommitReport

::: mosaica._reports.PagedReport
    options:
      show_bases: false

::: mosaica._commit.Finding
