# Contributing to Clementine

When contributing to Clementine, make sure that the changes you wish to make are in line with the project direction. If you are not sure about this, open an issue/discussion first, so we can discuss it.

Everyone is welcome to work on the project, and while we have lots of fun, it's a serious kind of fun. :^)

## Issue policy

Please do file any bugs you find, keeping the following in mind:

* One issue per bug. Putting multiple things in the same issue makes both discussion and completion unnecessarily complicated.
* Don't comment on issues just to add a joke or irrelevant commentary. Hundreds of people get notified about comments so let's keep them relevant.

## Code submission policy

Nobody is perfect, and sometimes we mess things up. That said, here are some good dos & dont's to try and stick to:

**Do:**

* Write in idiomatic Rust.
* Use formatting (see `just fmt`).
* Use the linter integrated (see `just lint`).
* Choose expressive variable, function and struct names. Make it as obvious as possible what the code is doing.
* Split your changes into separate, atomic commits (i.e. A commit per feature or fix, where the build, tests and the system are all functioning).
* Make sure your commits are rebased on the master branch.
* Wrap your commit messages at 72 characters.
* The first line of the commit message is the subject line, and must have the format "Category: Brief description of what's being changed". The category should be the name of a package, utility, etc.
  * Examples: `Memory`, `CPU`, `ARM`, `PPU`, `Everywhere`...
  * Several categories may be combined with `+`, e.g. `CPU+ARM+Memory: ...`
* Write the commit message subject line in the imperative mood ("Foo: Change the way dates work", not "Foo: Changed the way dates work").
* Write your commit messages in proper English, with care and punctuation.
* Squash your commits when making revisions after a patch review.
* Check the spelling of your code, comments and commit messages.

**Don't:**

* Touch anything outside the stated scope of the PR.
* Iterate excessively on your design across multiple commits.
* Use weasel-words like "refactor" or "fix" to avoid explaining what's being changed.
* End commit message subject lines with a period.
* Include commented-out code.
* Engage in excessive "feng shui programming" by moving code around without quantifiable benefit.
