#!/bin/bash
cd /home/giblfiz/hexwar
export PATH="/home/giblfiz/.npm-global/bin:$PATH"

# Show current time and previous note
echo "=== HEARTBEAT $(date) ===" >> heartbeat.log
echo "Previous note from last heartbeat:" >> heartbeat.log
cat /home/giblfiz/hexwar/heartbeat_note.txt 2>/dev/null >> heartbeat.log || echo "(no previous note)" >> heartbeat.log
echo "---" >> heartbeat.log

claude --dangerously-skip-permissions --print "Current time: $(date). Check the clock and note how long things have been running.

Read the note from your last heartbeat at /home/giblfiz/hexwar/heartbeat_note.txt

How's it going? Is anything stalled out? Does anything need to be kicked off? Any results in that need analysis? Sanity check run times? What needs to be done, or is everything as it should be and I can just snooze?

Before you finish, leave a note for your next heartbeat (in 10 min) at /home/giblfiz/hexwar/heartbeat_note.txt - include what's running, what to watch for, and any context the next heartbeat needs." 2>&1 | tee -a heartbeat.log
