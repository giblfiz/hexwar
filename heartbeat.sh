#!/bin/bash
cd /home/giblfiz/hexwar
claude --print "How's it going? Is anything stalled out? Does anything need to be kicked off? Any results in that need analysis? Sanity check run times? What needs to be done, or is everything as it should be and I can just snooze?" 2>&1 | tee -a heartbeat.log
