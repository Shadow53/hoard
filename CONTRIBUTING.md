This document is a work-in-progress.

# Table of Contents

- [Bug Reports](#bug-reports)
- [Feature Requests](#feature-requests)
- [Code](#code)
  - [Style](#style)
  - [Logging](#logging)

# Bug Reports

When creating a bug report, start by checking for similar bugs, *even if they are closed.* Closed
issues may contain the solution to your problem.

If the same issue was reported and closed due to lack of information, add your information to the existing issue and it
will get reopened for further review.

If no similar issues exist, create a new one with the following information:

- `hoard` version (commit if built from git).
- Steps to reproduce the issue.
- A minimal configuration file that causes the issue to be reproduceable.
- If the issue is not a crash or obvious error, a short description of what the expected behavior is.

This information will make it much easier for me to be able to help resolve your issue.

# Feature Requests

When submitting a feature request, first check the issue tracker for similar requests.

- If one exists and is open, add a thumbs up, heart, or other reaction to the first post to show your support.
- If one exists and is closed, check the reason for closing:
    - If marked `wontfix`, the feature request has been rejected. Please do not create a new request. If you feel there
      is a strong reason to reconsider *and* the thread is not locked, add your thoughts to the existing issue.
    - If not marked `wontfix`, the feature may have been implemented already! If that is the case, there will be a pull
      request linked somewhere in the issue's conversation.

If no similar request exists, you can submit a new one, with the following:

- The feature being requested.
- The benefit this feature will bring to users.
- Why this feature should be implemented in `hoard` and not externally.

If these are not included in the initial request, they will probably be asked for before the request will be considered.

# Code 

This section describes guidelines for how to write code that will be accepted.

## Style

Any code that is submitted via Pull Request needs to pass `clippy` checks and be formatted using `cargo fmt`. There are
CI checks in place that will fail if this is not the case.

## Logging

Any new code should be logged appropriately:

- `ERROR`: Fatal errors must be logged where they are created. If the error is built recursively or in a loop, this only
  applies to the final recursion/iteration.
- `WARN`: Non-fatal errors and potentially unexpected behavior (e.g. an operation being skipped) must be logged the same
  as fatal ones, but with this log level.
- `INFO`: Messages to inform the user of the high-level progress of the application get logged here. "High-level"
  currently means no more frequent than once per `Hoard`.
- `DEBUG`: More detailed messages to inform the user of some of the lower-level workings of the program. This generally
  means anything more specific than `INFO` but less specific than `TRACE`. A good rule of thumb is that multiple
  instances of a single `DEBUG` message should not take up more than one full-screen console window.
- `TRACE`: Very detailed messages that announce every non-trivial operation in the program.

These are some general rules used for writing log statements.

1. If a `hoard` library function may call another `hoard` library function and/or contains a loop, create a new span
   before that point with the necesssary context.
2. A span's context should only contain anything used in the immediate context. Items passed through to other library
   functions should be part of that more specific context, instead.
3. Let `hoard` library functions log themselves being called; create spans before calling for context, if necessary.
   That is, if there is a function call to `do_a_thing()`, the log message `"Doing a thing"` should come from *inside*
   `do_a_thing()`.
