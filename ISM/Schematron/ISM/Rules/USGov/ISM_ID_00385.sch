<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00385">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00385][Error] Attribute @ism:declassEvent requires use of attribute @ism:declassDate. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        CFR policies require that @ism:declassDate accompany @ism:declassEvent. Set context to any element 
        containing @ism:declassEvent attribute. Test if that element also has @ism:declassDate.
    </sch:p>
    <sch:rule id="ISM-ID-00385-R1" context="*[@ism:declassEvent]">
        <sch:assert test=".[@ism:declassDate]" flag="error" role="error">
            [ISM-ID-00385][Error]Attribute @ism:declassEvent requires use of attribute @ism:declassDate. 
        </sch:assert>
    </sch:rule>
</sch:pattern>