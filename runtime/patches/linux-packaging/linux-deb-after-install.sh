#!/bin/bash
# DSH Linux deb after-install script (v0.2.1-alpha.2 packaging).
#
# Mirrors electron-builder's default after-install template, with two changes
# that make the app launch from every context on Ubuntu 24.04/26.04:
#
# 1. The default template's user-namespace probe decides the chrome-sandbox
#    SUID bit from a check that runs as ROOT in postinst, while Chromium's
#    sandbox decision happens later in the unprivileged user context. On
#    systems with kernel.apparmor_restrict_unprivileged_userns=1 the probe
#    reports "userns works" but the unprivileged zygote cannot create one,
#    and a non-SUID helper makes the zygote FATAL ("cannot start").
# 2. electron-builder bundles a rendered AppArmor profile into
#    resources/apparmor-profile (flags=(unconfined) with the `userns,`
#    rule). Installing and loading it grants the binary unprivileged user
#    namespaces even under that restriction, so the zygote's user-namespace
#    sandbox works from terminal, menu, and systemd launches alike.
#
# chrome-sandbox is therefore kept root:0755 (never SUID - the SUID exec
# chain is unreliable when the install dir contains a space, "/opt/DeepSeek
# Harness"), and the AppArmor profile is installed below.
#
# `${executable}` and `${sanitizedProductName}` are substituted by
# electron-builder at build time (FpmTarget.writeConfigFile).

if type update-alternatives >/dev/null 2>&1; then
    # Remove previous link if it doesn't use update-alternatives
    if [ -L '/usr/bin/${executable}' -a -e '/usr/bin/${executable}' -a "`readlink '/usr/bin/${executable}'`" != '/etc/alternatives/${executable}' ]; then
        rm -f '/usr/bin/${executable}'
    fi
    update-alternatives --install '/usr/bin/${executable}' '${executable}' '/opt/${sanitizedProductName}/${executable}' 100 || ln -sf '/opt/${sanitizedProductName}/${executable}' '/usr/bin/${executable}'
else
    ln -sf '/opt/${sanitizedProductName}/${executable}' '/usr/bin/${executable}'
fi

# Keep the SUID helper deliberately NOT SUID (root:0755): the broken SUID
# exec path (space in the install dir) must not be selected by Chromium;
# the user-namespace sandbox enabled by the AppArmor profile below is used.
chown root:root '/opt/${sanitizedProductName}/chrome-sandbox' || true
chmod 0755 '/opt/${sanitizedProductName}/chrome-sandbox' || true

# Install apparmor profile. (Ubuntu 24+)
# First check if the version of AppArmor running on the device supports our profile.
# This is in order to keep backwards compatibility with Ubuntu 22.04 which does not support abi/4.0.
# In that case, we just skip installing the profile since the app runs fine without it on 22.04.
#
# Those apparmor_parser flags are akin to performing a dry run of loading a profile.
# https://wiki.debian.org/AppArmor/HowToUse#Dumping_profiles
#
# Unfortunately, at the moment AppArmor doesn't have a good story for backwards compatibility.
# https://askubuntu.com/questions/1517272/writing-a-backwards-compatible-apparmor-profile
if apparmor_status --enabled > /dev/null 2>&1; then
  APPARMOR_PROFILE_SOURCE='/opt/${sanitizedProductName}/resources/apparmor-profile'
  APPARMOR_PROFILE_TARGET='/etc/apparmor.d/${executable}'
  if apparmor_parser --skip-kernel-load --debug "$APPARMOR_PROFILE_SOURCE" > /dev/null 2>&1; then
    cp -f "$APPARMOR_PROFILE_SOURCE" "$APPARMOR_PROFILE_TARGET"

    # Updating the current AppArmor profile is not possible and probably not meaningful in a chroot'ed environment.
    # Use cases are for example environments where images for clients are maintained.
    # There, AppArmor might correctly be installed, but live updating makes no sense.
    if ! { [ -x '/usr/bin/ischroot' ] && /usr/bin/ischroot; } && hash apparmor_parser 2>/dev/null; then
      # Extra flags taken from dh_apparmor:
      # > By using '-W -T' we ensure that any abstraction updates are also pulled in.
      # https://wiki.debian.org/AppArmor/Contribute/FirstTimeProfileImport
      apparmor_parser --replace --write-cache --skip-read-cache "$APPARMOR_PROFILE_TARGET"
    fi
  else
    echo "Skipping the installation of the AppArmor profile as this version of AppArmor does not seem to support the bundled profile"
  fi
fi

if hash update-mime-database 2>/dev/null; then
    update-mime-database /usr/share/mime || true
fi

if hash update-desktop-database 2>/dev/null; then
    update-desktop-database /usr/share/applications || true
fi

# Refresh the GTK icon-theme cache so the freshly installed app icons
# (/usr/share/icons/hicolor, including the standard-size set) are visible
# without a re-login.
if hash gtk-update-icon-cache 2>/dev/null; then
    gtk-update-icon-cache -f /usr/share/icons/hicolor || true
fi
