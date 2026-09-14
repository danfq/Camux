#!/bin/sh
set -eu

package_name="camux-v4l2loopback"
package_version="0.15.4-camux1"
source_dir=${1:-}
camera_label=${2:-Camux Camera}
kernel_release=$(uname -r)
install_dir="/usr/src/${package_name}-${package_version}"

if [ "$(id -u)" -ne 0 ]; then
	echo "The Camux virtual-camera installer must run as an administrator." >&2
	exit 1
fi

if [ ! -f "${source_dir}/v4l2loopback.c" ] || [ ! -f "${source_dir}/dkms.conf" ]; then
	echo "The bundled v4l2loopback source is incomplete." >&2
	exit 1
fi

for command_name in dkms make install depmod modprobe sed; do
	if ! command -v "${command_name}" >/dev/null 2>&1; then
		echo "${command_name} is required to install the Camux virtual camera." >&2
		exit 1
	fi
done

if [ ! -d "/lib/modules/${kernel_release}/build" ]; then
	echo "Kernel headers for ${kernel_release} are required to install the Camux virtual camera." >&2
	exit 1
fi

dkms_state=$(dkms status -m "${package_name}" -v "${package_version}" 2>/dev/null || true)
if ! printf '%s\n' "${dkms_state}" | grep -q "^${package_name}/${package_version}"; then
	rm -rf -- "${install_dir}"
	install -d -m 0755 "${install_dir}"
	for source_file in COPYING Kbuild Makefile dkms.conf v4l2loopback.c v4l2loopback.h v4l2loopback_formats.h; do
		install -m 0644 "${source_dir}/${source_file}" "${install_dir}/${source_file}"
	done
	dkms add -m "${package_name}" -v "${package_version}"
fi

if ! printf '%s\n' "${dkms_state}" | grep -q "${kernel_release}.*installed"; then
	dkms build -m "${package_name}" -v "${package_version}" -k "${kernel_release}" --force
	dkms install -m "${package_name}" -v "${package_version}" -k "${kernel_release}" --force
fi

escaped_label=$(printf '%s' "${camera_label}" | sed 's/[\\"]/\\&/g')
install -d -m 0755 /etc/modprobe.d /etc/modules-load.d
{
	printf 'options camux_v4l2loopback exclusive_caps=1 max_openers=32 card_label="%s"\n' "${escaped_label}"
	printf 'install v4l2loopback modprobe camux_v4l2loopback\n'
} > /etc/modprobe.d/99-camux-v4l2loopback.conf
printf 'camux_v4l2loopback\n' > /etc/modules-load.d/camux-v4l2loopback.conf

for loaded_module in camux_v4l2loopback v4l2loopback; do
	if grep -q "^${loaded_module} " /proc/modules && ! modprobe -r "${loaded_module}"; then
		echo "The patched driver was installed, but the current driver is busy. Close every app using the Camux camera, then repair it again." >&2
		exit 20
	fi
done

depmod -a "${kernel_release}"
modprobe camux_v4l2loopback exclusive_caps=1 max_openers=32 "card_label=${camera_label}"

if [ "$(cat /sys/module/camux_v4l2loopback/version 2>/dev/null || true)" != "${package_version}" ]; then
	echo "The patched Camux driver was installed, but could not be loaded." >&2
	exit 1
fi
