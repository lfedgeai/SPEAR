import { useState } from 'react'
import { toast } from 'sonner'

import { getAdminToken } from '@/api/client'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

export default function SettingsPage() {
  const [token, setToken] = useState(() => getAdminToken())

  return (
    <div className="space-y-4">
      <div>
        <div className="text-lg font-semibold">Settings</div>
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          Local settings for this browser
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Admin token</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <Input
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="Optional Bearer token"
          />
          <div className="flex items-center gap-2">
            <Button
              onClick={() => {
                localStorage.setItem('ADMIN_TOKEN', token)
                toast.success('Saved')
              }}
            >
              Save
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                localStorage.removeItem('ADMIN_TOKEN')
                setToken('')
                toast.success('Cleared')
              }}
            >
              Clear
            </Button>
          </div>
          <div className="text-xs text-[hsl(var(--muted-foreground))]">
            Sent as Authorization: Bearer &lt;token&gt;.
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

