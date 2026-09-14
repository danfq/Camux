# Camux v4l2loopback dependency

This directory contains the source of
[v4l2loopback 0.15.4](https://github.com/v4l2loopback/v4l2loopback/tree/v0.15.4),
licensed under GPL-2.0 (see `COPYING`). It is bundled in Linux Camux packages and
installed through DKMS by the app's **Repair virtual camera** action.

Camux's `0.15.4-camux1` patch changes capture-side format and stream ownership
from a single token to a per-opener reference count. Each capture opener retains
the independent read position already provided by v4l2loopback, so Discord,
Zoom, OBS, and other V4L2 clients can stream from one Camux device concurrently.
Output ownership remains exclusive to Camux.

The built kernel module is named `camux_v4l2loopback`, keeping its DKMS output
separate from a distribution-provided `v4l2loopback` module. The two modules are
not loaded together because both expose the standard `/dev/v4l2loopback`
control interface. The installer redirects later `modprobe v4l2loopback` calls
to the Camux module so distribution boot configuration continues to work.

Upstream pull request
[#656](https://github.com/v4l2loopback/v4l2loopback/pull/656) is intentionally
not applied: it fixes output `DQBUF`, queue reset, and `STREAMOFF` wake-ups, but
does not remove the exclusive capture-reader token that blocks this use case.
