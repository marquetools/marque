<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00102">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00102][Error] The attribute @ism:DESVersion in the namespace urn:us:gov:ic:ism must be specified.   
        
        Human Readable: The data encoding specification version must be specified.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Make sure that the attribute @ism:DESVersion is specified.
    </sch:p>
    <sch:rule id="ISM-ID-00102-R1" context="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]">
        <sch:assert test="some $element in descendant-or-self::node() satisfies $element/@ism:DESVersion" flag="error" role="error">
            [ISM-ID-00102][Error] The attribute @ism:DESVersion in the namespace urn:us:gov:ic:ism must be specified.
            
            Human Readable: The data encoding specification version must be specified.
        </sch:assert>
    </sch:rule>
</sch:pattern>