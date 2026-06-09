# Building the Windows Installer in the Cloud (GitHub Actions)

This produces a **Windows `.msi` and `-setup.exe`** without owning a Windows
machine, using GitHub's free Windows runners. The workflow also builds the Linux
`.AppImage`/`.deb` in the same run.

> This is a **developer/operator** task (done once by you), not something the
> client does. The client only receives the finished installer + `INSTALL_GUIDE.md`.

The workflow file is already in the repo at
[.github/workflows/build-installers.yml](.github/workflows/build-installers.yml).

---

## One-time: put the project on GitHub

If the desktop app isn't on GitHub yet:

1. Create a new **private** repository at https://github.com/new (e.g.
   `ticketing-desktop`). Don't add a README/.gitignore — we already have files.

2. From inside the `ticketing-desktop` folder, run:

   ```bash
   git init
   git add .
   git commit -m "Phase 1 printer prototype + CI build workflow"
   git branch -M main
   git remote add origin https://github.com/<your-account>/ticketing-desktop.git
   git push -u origin main
   ```

   > The `.github/workflows/build-installers.yml` file must be pushed for the
   > Actions tab to show the workflow. (It's included — just make sure it's not
   > excluded by a `.gitignore`.)

---

## Run the build

1. On GitHub, open the repo → **Actions** tab.
2. If prompted, click **"I understand my workflows, enable them"**.
3. In the left sidebar, click **"Build installers"**.
4. Click **"Run workflow"** (top-right) → keep branch `main` → **Run workflow**.
5. Wait ~10–15 minutes. Two jobs run in parallel: `windows-latest` and
   `ubuntu-22.04`. Green check = success.

> **Automatic builds on release:** the workflow also runs when you push a version
> tag, so you can cut a release with:
> ```bash
> git tag v0.1.0 && git push origin v0.1.0
> ```

---

## Download the installers

1. Click the finished workflow run.
2. Scroll to the **"Artifacts"** section at the bottom.
3. Download:
   - **`windows-installers`** → contains the `.msi` and `...-setup.exe`
     → send to Windows clients.
   - **`linux-installers`** → contains the `.AppImage` and `.deb`
     → send to Linux clients.

Each artifact downloads as a `.zip`; unzip it to get the installer file.

---

## What to send the client

Send them **one installer for their OS** plus the easy guide:

| Client OS | File(s) from the artifact | Guide |
|-----------|---------------------------|-------|
| Windows | `...-setup.exe` (or `.msi`) | `INSTALL_GUIDE.md` |
| Linux | `...amd64.AppImage` (or `.deb`) | `INSTALL_GUIDE.md` |

The client installs **only** that file — no Rust, Node, build tools, or SQLite.

---

## Notes

- **Code signing:** these installers are **not** code-signed, so Windows shows a
  *"Windows protected your PC"* prompt (the client clicks *More info → Run
  anyway*). For a polished client-facing release later, we can add an
  Authenticode certificate to the workflow so that prompt disappears. Not needed
  for Phase 1 testing.
- **Cost:** public repos get unlimited free Actions minutes; private repos get a
  monthly free allotment that is far more than enough for occasional builds.
- **Why not build Windows locally here:** this project was developed on Linux.
  Producing a reliable Windows installer needs a Windows toolchain — the CI
  runner provides exactly that, verified by Microsoft's own image.
