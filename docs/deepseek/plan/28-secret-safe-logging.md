# Plan: safe diagnostic logging

Goal: remove exposed tokens, API keys, and credentials from new diagnostic logs while preserving useful metadata.

First audit backend logging and add a small limited-mask helper; then separately check the frontend debug console. Do not change old log files, startup DTO, or add an export command.
