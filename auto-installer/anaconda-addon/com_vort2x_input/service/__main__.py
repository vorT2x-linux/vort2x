from pyanaconda.modules.common import init

init()

from com_vort2x_input.service.vort2x import VorT2xService

service = VorT2xService()
service.run()
