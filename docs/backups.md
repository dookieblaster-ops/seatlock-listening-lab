# Version control backups — owner inspection

This page is for the owner. Agents keep the checks and the workflow file true; you turn on the GitHub switches only you can reach.

## What is already in the repository

1. **GitHub is the primary copy** of the private repo `dookieblaster-ops/seatlock-listening-lab`.
2. **Automatic Check on GitHub** (`.github/workflows/check.yml`) runs `./check.sh` on every pull request and every push to `main`. A red check means something is broken.
3. **`tools/mirror-backup.sh`** can push every branch and tag to a second remote named `backup`, once you add that remote.

## What you need to inspect / turn on

Open the repo on GitHub while signed in as the owner.

### 1. Allow Actions (required for the Check) — done 2026-11-23

Actions is already on: the Check workflow runs on every push and pull request. You do not need to change this unless you later turn Actions off.

If you ever need to re-enable it:

1. **Settings → Actions → General**
2. Under "Actions permissions", choose **Allow all actions and reusable workflows** (or at least allow actions created by GitHub).
3. Save.

### 2. Protect `main` (strongly recommended)

1. **Settings → Branches → Add branch protection rule** (or **Rules → Rulesets** if GitHub offers that).
2. Branch name pattern: `main`
3. Turn on at least:
   - **Do not allow force pushes**
   - **Do not allow deletions**
4. If GitHub lets you without upgrading:
   - **Require status checks to pass before merging**, and require the check named **`./check.sh`** (it appears after the first green Actions run).
5. If GitHub asks you to upgrade to **Pro** for these rules on a private repo: that is a paid GitHub plan. You can skip this step and still keep the second backup remote below. Pro is worth it if you want GitHub itself to refuse a force-push to `main`.

### 3. Second backup remote (recommended)

GitHub is already a remote backup of every laptop clone. A **second** private copy protects you if the GitHub account or repo is ever lost.

1. Create an empty **private** repository somewhere you control (another GitHub account, GitLab, or similar). Do not initialize it with a README.
2. Tell the agent the clone URL, **or** on a machine that already has this repo:

```bash
git remote add backup <paste-the-empty-repo-https-or-ssh-url>
./tools/mirror-backup.sh
```

3. Re-run `./tools/mirror-backup.sh` after big known-good moments (or ask an agent to). It needs write access to that second repo.

Do **not** paste access tokens into chat. Use GitHub/GitLab's normal login or a deploy key on your own machine, or ask the agent only for the **URL** of an empty private repo you already created.

## What this does not do

- It does not replace the physics checks themselves.
- It does not back up files that never entered git (local prints, downloads outside the repo).
- A red Check on `main` after a direct push still means "fix before trusting"; protection rules are what *block* the bad merge when GitHub allows them.

## After the first green Check

Tell the agent "Actions is on" (and, if you set it, "main is protected" / "backup remote URL is …"). The agent can then confirm the Check is green and wire the backup remote if you gave a URL.
