# The gate

The reviewer describes; the board decides. The reviewer gives no score and no merge advice. Every round it answers
one fixed question, "Does the change do what the task asked?" (`meets`), and lists its findings. Each finding has a
severity, a subject (and a [short title](titles.md) of it), where it is, a proof (what the reviewer actually ran or
saw) and whether it was there before the task (`preexisting`). The severity says what the fault does:

| Severity | Means |
| --- | --- |
| `critical` | only one of four: a fault a user would hit in ordinary use; data that is lost or wrong; something unsafe; a requirement the task states plainly that is not built. A missing picture, a line of a document, or proof that cannot be had on this machine is not critical |
| `major` | a real fault in an unusual case; a claim that is not proven (a fix not shown to work, a test that cannot catch its bug); a document that states something false |
| `minor` | correct but could be better: wording, naming, a small inaccuracy, a narrow gap |
| `trivial` | a note; nothing to do |

The board then applies fixed rules to what came back:

- The reviewer's own findings and their severities decide. `meets: false` adds a critical finding of the board's
  own ("The change does not do what the task asked") only when the reviewer listed nothing at all. Beside real
  findings the board adds nothing: that line used to be 67 of 292 findings, and 28 of 76 failed rounds had no
  critical finding of the task's own.
- A critical finding with an empty proof is treated as major.
- A pre-existing finding never sends the work back, and never becomes a task. A round whose findings are all of
  that kind passes, whatever `meets` says. It is a note: the review lists it
  as `pre-existing`, the agent is told to leave it, and nothing else is done with it. Add a task yourself for one
  you want fixed.
- The work **passes** when no finding of its own sits at or above `review.mustFix` (`major` by default). A pass
  is what "may merge" means everywhere: a task with `merge.by: itself` merges it ([merging by itself](#merging-by-itself)),
  an approved merge that was repaired goes through, and the board says `Pass`.
- A **fail** goes back to the agent by itself, at most `review.rounds` times: the findings at or above the gate as
  required, the rest as optional. When the cap is reached and it still fails, it stops and waits for you; with a
  fallback agent set (`worker.fallback`) the next agent on that list takes the work over first, with the open
  findings and rounds of its own ([the fallback agent](fallback.md)).
- From the second round on the reviewer gets the earlier findings and only checks that each is fixed and that the
  fixes broke nothing. It may add a new finding only if it is critical, with proof; anything smaller it notices
  late is left out (a pre-existing fault is still noted). A reply from you starts a full review again.
- `review.rounds 0` sends nothing back by itself: findings wait for you (`Ctrl+R` sends them). The review is
  still judged at `review.mustFix`, and a failed one never merges by itself.

## Merging by itself

Finished work waits for your yes (`merge.by: you`). `merge.by: itself` makes it merge by itself instead, the moment
it is green:

- **Green** means: the check passed, the review passed the gate (or the task is not reviewed at all), the branch is up
  to date with main with no conflict, and the pull request has no failed check or requested change.
- **Only the reviewed commit merges.** The board remembers the commit the review saw and merges exactly that. A
  commit that reached the branch or the pull request afterwards stops the merge and is checked and reviewed first.
  (GitHub, asked right after the board's own push, may still name the commit before it. That is not such a commit:
  while the branch at the remote is the reviewed one, the board asks again.)
- **Main moved since the review: the task catches up and merges, with nobody asked.** The board merges main into
  the branch itself and runs the check on the result. When that merge is clean and the check passes, the review
  still stands and the task merges at once: such a merge of the board's own counts as
  reviewed. Any other new commit does not. When the merge conflicts, or the check fails after it, the task goes
  back to its agent to repair, and what the agent made of it is reviewed before it may merge, the
  conflicts it resolved included (that review looks at what came after the last one only). With
  `merge.catchUp merge` a waiting task takes main in only here, once, right before the merge.
- **One at a time.** Every merge moves main under the tasks behind it, so a repo's tasks merge in a line: one
  catches up, is checked and merges, then the next, in the order they became ready. A task that waits says
  `Waiting to merge: <id> goes first`; it is not brought up to date before its turn, so it catches up once, not
  once for every merge ahead of it. A task that must be reviewed again leaves the line for that and joins it again.
  The line moves with the board closed too.
- **What needs you still stops and asks**: a review that fails the gate after its rounds, checks on GitHub that
  still fail after the rounds or never finish (`merge.checks`), a merge that failed, a
  login, a question or a request for permission from the agent, a checkout of yours on another branch. A task you
  stepped into after it asked you (a reply to its agent, a turn in its terminal, delegating it again) asks you
  again too, once; `set merge.by itself --task <id>` on that task says you do not need to be asked. Sending the
  board's own words back to the agent (the review, or what went wrong: `Ctrl+R`, `review`, `fix`) is not stepping
  in: you added nothing of your own, the repair is checked and reviewed, and it merges by itself when it passes.
  (An older version did count it, and left such tasks waiting for a yes. Such a task is let go only where that can
  be proven from what its agent was told: its task once, before any review, and since then nothing but reviews of
  its own, letter for letter as the board words them. With anything else in that conversation, a failure's details
  sent back included (they are not kept, so they cannot be told from words of yours), or a conversation the board
  cannot read (only Claude Code's can be), the task asks you once more. A hold made after the board stopped
  counting a send-back is never lifted this way.)
- **Set it like any other setting**: `herdr-projects set merge.by itself` for every task, `--repo <name>` for one
  repo's, `--task <id>` or `delegate <id> --merge.by itself` for one task, `unset merge.by` to take a choice back;
  or tell the Coordinator "auto-merge this one" or "always auto-merge". The task panel shows it as `Merge`.
- **It counts from the moment you change it**, for tasks under way too. Turned on: a running task merges when it
  turns green, and a green task that already waits for your yes merges at once (turned on for every task, that is
  every such task: look at what waits first). Turned off: a task that waits to merge by itself, for GitHub's checks
  or a merge queue, goes back to waiting for your yes, and so does one whose merge the board had started but not
  yet made (the setting is asked again right before the merge).

A repo's own committed file cannot switch this on. `"land": "auto"` in `.herdr-projects.json` used to; that file
now holds seven facts only ([settings](settings.md)), so code an agent writes can never make its own work merge
unasked. To merge by itself in one repo, say so yourself: `set merge.by itself --repo <name>`. `show <id>` says what
a task really runs on, and where each value comes from.

Findings below the gate still show on the review page, in the task panel and in a comment on the pull request,
marked as not required.

An error, a usage limit, a cut-off or a refusal is saved as "Agent stopped" with the real text (an error of the
provider that passes by itself is retried first: [A turn cut off by the provider](provider-errors.md)); a login error makes
the task "Needs a login", and that comes first: older failure details are not sent and pull-request feedback does not
wake the agent until you have logged in and then replied, pressed `Ctrl+R` or typed in the agent's terminal. A turn
only blocks on you when the agent ran `report … --state blocked`; a final message that merely ends in a question
goes on to check and review, and you still approve at the end
([An agent that waits for you is not finished](waiting-for-you.md)).
