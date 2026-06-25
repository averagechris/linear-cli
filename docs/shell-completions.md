# Shell Completions

Generate static completions, or dynamic completions when you want Linear-backed values such as teams, users, and statuses.

```bash
linear completions static bash > ~/.bash_completion.d/linear
linear completions static zsh > ~/.zfunc/_linear
linear completions static fish > ~/.config/fish/completions/linear.fish
linear completions static powershell > linear.ps1

linear completions dynamic bash
linear completions dynamic zsh
```

Shell setup is standard for each shell: put the generated file on the shell completion path and reload the shell.
