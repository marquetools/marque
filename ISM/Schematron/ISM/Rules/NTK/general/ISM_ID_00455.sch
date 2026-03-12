<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00002 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00455">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00455][Error] ntk:RequiresAnyOf and ntk:RequiresAllOf must contain ntk:AccessProfileList.
        
        Human Readable: ntk:RequiresAnyOf and ntk:RequiresAllOf must have the child element ntk:AccessProfileList.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule ensures that ntk:AccessProfileList exist as a child element of ntk:RequiresAnyOf and ntk:RequiresAllOf.
    </sch:p>
    <sch:rule id="ISM-ID-00455-R1" context="ntk:RequiresAnyOf|ntk:RequiresAllOf">
        <sch:assert test="ntk:AccessProfileList" flag="error" role="error">
            [ISM-ID-00455][Error] ntk:RequiresAnyOf and ntk:RequiresAllOf must contain ntk:AccessProfileList.            
            
            Human Readable: ntk:RequiresAnyOf and ntk:RequiresAllOf must have the child element ntk:AccessProfileList.
        </sch:assert>
    </sch:rule>
</sch:pattern>
