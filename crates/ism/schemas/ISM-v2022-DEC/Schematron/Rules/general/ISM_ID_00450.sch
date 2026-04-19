<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00450">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00450][Warning] @arh:DESVersion is a DEPRECATED attribute.
        
        Human Readable: ARH DESVersion is a DEPRECATED attribute.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If @arh:DESVersion exists, provide a warning that it is a DEPRECATED attribute.
    </sch:p>
    <sch:rule id="ISM-ID-00450-R1" context="*[@arh:DESVersion]">
        <sch:assert test="not(@arh:DESVersion)" flag="warning" role="warning">
            [ISM-ID-00450][Warning] @arh:DESVersion is a DEPRECATED attribute. Found arh:DESVersion=<sch:value-of select="./@arh:DESVersion"/>
            
            Human Readable: ARH DESVersion is a DEPRECATED attribute.
        </sch:assert>
    </sch:rule>
</sch:pattern>