<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:ism="urn:us:gov:ic:ism" xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00449">
    <sch:p ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00449][Error] The ARH elements cannot be used as root elements.
        
        Human Readable: ARH is not designed to stand-alone and therefore should never
        be used as a root element.
    </sch:p>
    <sch:p ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Ensure that arh:Security or arh:ExternalSecurity are not used as the root element.
    </sch:p>
    <sch:rule id="ISM-ID-00449-R1" context="/arh:*">
        <sch:assert test="false()" flag="error" role="error">
            [ISM-ID-00449][Error] The ARH elements cannot be used as root elements.
            
            Human Readable: ARH is not designed to stand-alone and therefore should never
            be used as a root element.
        </sch:assert>
    </sch:rule>
</sch:pattern>