<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00489">
   <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
      [ISM-ID-00489][Error] Use of the Enterprise Role Restrictive access policy requires the ROLE Profile DES.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
      If ntk:AccessProfile has an ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:enterprise:role:restrictive', 
      then ntk:ProfileDes must be 'urn:us:gov:ic:ntk:profile:role'.
   </sch:p>
   <sch:rule id="ISM-ID-00489-R1"
             context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:enterprise:role:restrictive']/ntk:ProfileDes">
      <sch:assert test=". = 'urn:us:gov:ic:ntk:profile:role'"
                  flag="error"
                  role="error">
         [ISM-ID-00489][Error] Use of the Enterprise Role Restrictive access policy requires the ROLE Profile DES.
      </sch:assert>
   </sch:rule>
</sch:pattern>
