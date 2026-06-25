---
name: linear-uploads
description: Download attachments and images from Linear issues. Use when fetching screenshots, images, or file attachments from Linear comments or descriptions.
allowed-tools: Bash Read
---

# Uploads

Download Linear upload URLs so agents can inspect attachments/images.

## Start here

```bash
linear up fetch URL -f /tmp/linear-upload.png
linear up fetch URL > file.bin
linear cm list LIN-123 --output json --compact --fields body,url
```

## Agent notes
- Use `-f` when the next tool needs a path to an image/file.
- Comments and issue bodies may contain the upload URLs to fetch.
- Full syntax and less-common flags: `linear up fetch --help`.
