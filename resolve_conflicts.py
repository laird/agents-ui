import re
import sys

def resolve_conflicts(filepath, strategy='both'):
    """Resolve git merge conflicts by keeping content from both sides."""
    with open(filepath, 'r') as f:
        content = f.read()
    
    if '<<<<<<' not in content:
        print(f"No conflicts in {filepath}")
        return False
    
    result = []
    lines = content.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith('<<<<<<<'):
            head_lines = []
            branch_lines = []
            i += 1
            # Collect HEAD content
            while i < len(lines) and not lines[i].startswith('======='):
                head_lines.append(lines[i])
                i += 1
            i += 1  # skip =======
            # Collect branch content
            while i < len(lines) and not lines[i].startswith('>>>>>>>'):
                branch_lines.append(lines[i])
                i += 1
            i += 1  # skip >>>>>>>
            # Keep both
            result.extend(head_lines)
            result.extend(branch_lines)
        else:
            result.append(line)
            i += 1
    
    with open(filepath, 'w') as f:
        f.write('\n'.join(result))
    
    return True

files = [
    'src/adapter/claude.rs',
    'src/app.rs',
    'src/config/keybindings.rs',
    'src/config/persistence.rs',
    'src/config/shortcuts.rs',
    'src/github.rs',
    'src/main.rs',
    'src/model/issue.rs',
    'src/model/swarm.rs',
    'src/tmux/session.rs',
    'src/ui/agent_view.rs',
    'src/ui/feedback_dialog.rs',
    'src/ui/help_overlay.rs',
    'src/ui/issue_detail.rs',
    'src/ui/issue_view.rs',
    'src/ui/new_swarm.rs',
    'src/ui/repo_view.rs',
    'src/ui/repos_list.rs',
    'src/ui/shortcuts_viewer.rs',
    'src/ui/swarm_view.rs',
]

for f in files:
    changed = resolve_conflicts(f)
    if changed:
        print(f"Resolved: {f}")

print("Done!")
