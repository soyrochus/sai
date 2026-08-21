# Connect an AI Coding Assistant

Every "AI collaboration script" in this course is written as plain English, deliberately independent of any one product — the course outcome is a discipline (bounded prompts, reviewed diffs, evidence before trust), not fluency in one vendor's interface. But if you have never used an agentic coding tool before, "paste this into your AI assistant" is not obviously actionable. This page is the one place the course gets concrete: how to install, sign in to, and send a prompt through three widely used options. Pick one — you do not need all three, and nothing later in the course assumes which you chose.

Whichever you install, open it with `sai-course` — the project you create in [the curriculum's setup step](README.md#before-you-begin) — as the open folder or workspace root, not this `sai` reference repository. The assistant needs to read and edit the project you are actually building.

## Claude Code

[claude.com/product/claude-code](https://claude.com/product/claude-code) — an agentic assistant from Anthropic. Its terminal CLI is the primary surface, with native VS Code and JetBrains extensions, a web and mobile client, and a Slack integration, so it "meets developers where they code" rather than requiring one environment. It maps and explains a codebase through agentic search, proposes multi-file edits as a reviewable diff, and runs terminal commands — installing dependencies, running tests, opening pull requests — on request. That combination is a good match for this course's "propose a plan → inspect the diff → compile → test" loop.

1. In VS Code, open the Extensions view (`Cmd+Shift+X` on Mac, `Ctrl+Shift+X` on Windows/Linux), search for **Claude Code** (publisher: Anthropic), and install it.
2. Open the panel: click the Spark icon in the editor toolbar (appears once a file is open), or open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) and run **Claude Code: Open in New Tab**.
3. Sign in with any paid Claude plan (Pro, Max, Team, or Enterprise) or a Claude Console account — no API key required.
4. Paste a chapter's "AI collaboration script" prompt into the prompt box at the bottom of the panel and send it.

The extension bundles its own CLI for the panel. If you would rather work from a terminal, install the standalone CLI separately and run `claude` inside `sai-course`; the same prompts work there unchanged.

## GitHub Copilot

[github.com/features/copilot/ai-code-editor](https://github.com/features/copilot/ai-code-editor) — the most widely deployed assistant, natively integrated into VS Code (and available in over a dozen other environments). Its default Chat view is a conversational, code-suggestion-first experience, but it also has an Agent mode that autonomously edits across multiple files, runs tests, and validates results, plus a Plan mode that shows the blueprint for review before the agent starts. An `AGENTS.md` file lets you share project-specific instructions so every agent session follows the same conventions.

1. Hover over the Copilot icon in the VS Code status bar and select **Use AI Features** — this installs the required extensions (GitHub Copilot and GitHub Copilot Chat) automatically.
2. Sign in with your GitHub account. A free tier with a limited monthly allowance exists if you do not have a paid Copilot plan.
3. Open the Chat view with `Ctrl+Alt+I` (Windows/Linux) or `Ctrl+Cmd+I` (Mac).
4. Paste a chapter's prompt into the chat box and send it. For the more autonomous, multi-file behavior this course's later chapters assume, look for an agent mode toggle in the chat view, or open the dedicated Agents window via the VS Code title bar.

## Codex

[openai.com/codex](https://openai.com/codex/) — OpenAI's coding agent. Like Claude Code, its primary surface is a lightweight terminal CLI, with IDE extensions for VS Code, Cursor, and Windsurf, a desktop app, and a cloud-hosted agent at chatgpt.com/codex for handing off larger tasks. As a full agent it reads files, runs commands, installs dependencies, executes tests, and iterates on failures rather than only suggesting a diff.

1. Open VS Code's Quick Open (`Cmd+P` / `Ctrl+P`), paste the install command from the [Codex extension page](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt), and press Enter.
2. Sign in with a ChatGPT account (Plus, Pro, Business, Edu, or Enterprise).
3. Open the extension's panel from the Activity Bar and paste a chapter's prompt.

On macOS, you can alternatively open the ChatGPT desktop app and choose **Work with VS Code** to connect the two. If you would rather work from a terminal, install the standalone CLI with `npm install -g @openai/codex` (or the installer script or Homebrew) and run `codex` inside `sai-course`; the same prompts work there unchanged.

## Any of them, any chapter

Once one of these is open and pointed at `sai-course`, every "AI collaboration script" blockquote in every chapter is something you paste as-is — none of them contain tool-specific syntax. What differs between tools is only the review step: how each one shows you a diff before it touches a file. Read that diff regardless of which tool produced it; that habit, not the tool, is what the course is teaching.
