#!/bin/bash -euET

wrap_args=(--unshare-user --unshare-ipc --unshare-pid --unshare-uts --unshare-cgroup)

# If you want to configure this script, do it by via wrap_args.d.
# See an example in this directory.
for filename in ~/.config/rua/wrap_args.d/*.sh ; do
  if test -e "$filename"; then source "$filename"; fi
done

exec nice -n19 \
  ionice -c idle \
  bwrap \
  --new-session --die-with-parent \
  --ro-bind /usr /usr \
  --ro-bind /opt /opt \
  --ro-bind /etc /etc \
  --ro-bind /boot /boot \
  --ro-bind /var /var \
  --perms 000 --dir /root \
  --dir /mnt \
  --dir /media \
  --dir /srv \
  --symlink usr/bin /bin \
  --symlink usr/bin /sbin \
  --symlink usr/lib /lib \
  --symlink usr/lib /lib64 \
  --dev /dev \
  --proc /proc \
  --ro-bind /sys /sys \
  --tmpfs /tmp \
  --tmpfs /run \
  --tmpfs /var/run \
  --tmpfs /var/tmp \
  --perms 0700 --tmpfs "$XDG_RUNTIME_DIR" \
  --tmpfs "$HOME" \
  --ro-bind /etc/resolv.conf /etc/resolv.conf \
  --ro-bind-try "${GNUPGHOME:-$HOME/.gnupg}/pubring.kbx" "${GNUPGHOME:-$HOME/.gnupg}/pubring.kbx" \
  --ro-bind-try "${GNUPGHOME:-$HOME/.gnupg}/pubring.gpg" "${GNUPGHOME:-$HOME/.gnupg}/pubring.gpg" \
  --clearenv \
  --setenv XDG_RUNTIME_DIR "$XDG_RUNTIME_DIR" \
  --setenv PATH "$PATH" \
  --setenv USER "$USER" \
  --setenv LOGNAME "$LOGNAME" \
  --setenv TERM "$TERM" \
  --setenv HOME "$HOME" \
  --setenv GNUPGHOME "${GNUPGHOME:-$HOME/.gnupg}" \
  --setenv LANG "$LANG" \
  --setenv PKGDEST "$PKGDEST" \
  --setenv SRCDEST "$SRCDEST" \
  --setenv SRCPKGDEST "$SRCPKGDEST" \
  --setenv LOGDEST "$LOGDEST" \
  --setenv BUILDDIR "$BUILDDIR" \
  --setenv FAKEROOTDONTTRYCHOWN 1 \
  "${wrap_args[@]}" \
  --seccomp 3 3< "$RUA_SECCOMP_FILE" \
  "$@"
