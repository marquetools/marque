<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00007 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00400">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00400][Error] The attribute @ntk:externalReference must be set to [true] when ntk:ExternalAccess element is used.
        
        Human Readable: If ntk:ExternalAccess element is used, then the attribute @ntk:externalReference must have a value of true.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Make sure the @ntk:externalReference attribute is specified with a value of [true] 
        when the ntk:ExternalAccess element is used.
    </sch:p>    
    <sch:rule id="ISM-ID-00400-R1" context="ntk:ExternalAccess">
        <sch:assert test="@ntk:externalReference=true()" flag="error" role="error">
            [ISM-ID-00400][Error] The attribute @ntk:externalReference must be set to [true] 
            when ntk:ExternalAccess element is used.
            
            Human Readable: If ntk:ExternalAccess element is used, then the attribute @ntk:externalReference 
            must have a value of true.
        </sch:assert>
    </sch:rule>
</sch:pattern>
