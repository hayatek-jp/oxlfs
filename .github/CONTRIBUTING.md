# Contribution Guidelines
Thank you for considering contributing to this project.  
To ensure smooth project management, I have established contribution guidelines. I kindly ask you to read them before making a contribution.  

> [!IMPORTANT]
> By contributing, you are automatically deemed to have agreed to these guidelines!


## General
1. You should use English in this project.


## Issues

1. Do not use Issues to ask questions. Instead, use Discussions.
2. Do not post vulnerability reports in Issues. If you do, they will be deleted without prior notice. Instead, use the Security Advisories.
3. Use the provided template when creating an Issue.
4. Check if a similar Issue already exists before creating a new one.
5. When submitting a bug report, include details about your environment and provide the full traceback.
6. For more details, refer to the explanations in the template.


## Pull Requests

1. Before writing code, check existing Issues and Pull Requests. Someone else may have already started working on the same thing.
2. If your code is not related to an existing Issue, I recommend creating an Issue for bug fixes or a Draft Pull Request for new features.  Otherwise, someone else might finish writing similar code before you, or your contribution might not be accepted.  To avoid wasting time, post about what you're planning to implement and get confirmation in advance.
3. Do not submit security patches. (except when the information is already publicly available.)  If you do, they will be deleted without prior notice. Always submit them via the Security Advisories.  If you're unsure whether your patch is security-related, send a private [email to the maintainer](mailto:dev@hayatek.jp) for confirmation.
4. Use the provided template when creating a Pull Request.
5. Do not submit untested code.
6. Follow the commit guidelines.
7. For more details, refer to the explanations in the Pull Request template.


## Commits

1. Ensure that your commits do not have any legal or licensing issues. Every commit must be signed off.
2. Write clear and concise commit messages.
3. Keep commits as small as possible. Do not include multiple changes in a single commit.
4. When a series of commits belong to a specific Pull Request, avoid using revert commits whenever possible. Instead, use `git rebase` to drop them. However, this rule does not apply when working with multiple contributors.
5. Follow these commit message conventions:
  a) Start messages with a capital letter.
  b) Prefix commit messages with a concise description of what the commit does.

    | prefix      | description                                                     |
    | ----------- | --------------------------------------------------------------- |
    | add         | Add something                                                   |
    | feat        | Add new features                                                |
    | change      | Fixes due to specification change                               |
    | update      | Update something (Not dependency updates)                       |
    | upgrade     | Upgrade project versions                                        |
    | deps        | Add/Update dependencies                                         |
    | deps(dev)   | Add/Update dev-dependencies                                     |
    | deps(build) | Add/Update build-dependencies                                   |
    | fix         | Fix something                                                   |
    | hotfix      | Fix something critical (e.g. security issues)                   |
    | rename      | Rename files                                                    |
    | remove      | Remove something                                                |
    | revert      | Revert something (Only when automatic revert is not sufficient) |
    | disable     | Disable something temporarily                                   |
    | refactor    | Refactorings                                                    |
    | improve     | Improve something                                               |
    | perf        | Performance updates                                             |
    | style       | Fix coding styles                                               |
    | test        | About test codes                                                |
    | ci          | About CI |
    | docs        | About docs                                                      |
    | chore       | Chores                                                          |

    **Exceptions**:
      - Merge commits
      - Revert commits (Only when automatic revert)
  c) If you need to commit work-in-progress code, prefix the commit message with `[WIP]`. However, before submitting a Pull Request, you must `amend` or `squash` these commits.

> [!TIP]
> The commit message rules can be complex, but they does not have to be perfect. The maintainer will adjust them if needed, so don't overthink it—just contribute!

