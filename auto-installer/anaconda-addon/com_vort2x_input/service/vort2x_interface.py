from dasbus.server.interface import dbus_interface

from pyanaconda.modules.common.base import KickstartModuleInterface

from com_vort2x_input.service.constants import VORT2X


@dbus_interface(VORT2X.interface_name)
class VorT2xInterface(KickstartModuleInterface):
    """DBus interface for the VorT2x transition-driver add-on."""
