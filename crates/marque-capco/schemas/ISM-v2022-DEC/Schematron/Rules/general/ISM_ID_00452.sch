<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00452">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00452][Warning] @ntk:DESVersion is a DEPRECATED attribute.
        
        Human Readable: NTK DESVersion is a DEPRECATED attribute.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If @ntk:DESVersion exists, provide a warning that it is a DEPRECATED attribute.
    </sch:p>
    <sch:rule id="ISM-ID-00452-R1" context="*[@ntk:DESVersion]">
        <sch:assert test="not(@ntk:DESVersion)" flag="warning" role="warning">
            [ISM-ID-00452][Warning] @ntk:DESVersion is a DEPRECATED attribute.
            
            Human Readable: NTK DESVersion is a DEPRECATED attribute.
        </sch:assert>
    </sch:rule>
</sch:pattern>