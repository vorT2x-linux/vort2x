from pyanaconda.core.dbus import DBus
from pyanaconda.modules.common.base import KickstartService
from pyanaconda.modules.common.containers import TaskContainer

from com_vort2x_input.service.constants import VORT2X
from com_vort2x_input.service.installation import (
    ConfigureKernelArgumentsTask,
    InstallBluetoothFirmwareTask,
    InstallGuidedInstallerTask,
    InstallTransitionDriversTask,
    InstallWifiFirmwareTask,
)
from com_vort2x_input.service.vort2x_interface import VorT2xInterface


class VorT2xService(KickstartService):
    """Install the temporary T2 input drivers into the target system."""

    def publish(self):
        TaskContainer.set_namespace(VORT2X.namespace)
        DBus.publish_object(VORT2X.object_path, VorT2xInterface(self))
        DBus.register_service(VORT2X.service_name)

    def install_with_tasks(self):
        return [
            InstallWifiFirmwareTask(),
            InstallBluetoothFirmwareTask(),
            InstallGuidedInstallerTask(),
            InstallTransitionDriversTask(),
        ]

    def configure_bootloader_with_tasks(self, kernel_versions):
        return [ConfigureKernelArgumentsTask()]
